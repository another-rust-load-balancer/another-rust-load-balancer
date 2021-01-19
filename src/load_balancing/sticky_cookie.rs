use crate::middleware::{RequestHandlerChain, RequestHandlerContext};

use super::{create_context, LoadBalancingContext, LoadBalancingStrategy};
use async_trait::async_trait;
use cookie::{Cookie, SameSite};
use hyper::{
  header::{Entry, HeaderValue, COOKIE, SET_COOKIE},
  http::response,
  Body, Request, Response,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct StickyCookieConfig {
  pub cookie_name: String,
  pub secure: bool,
  pub http_only: bool,
  pub same_site: SameSite,
}

#[derive(Debug)]
pub struct StickyCookie {
  pub config: Arc<StickyCookieConfig>,
  pub inner: Box<dyn LoadBalancingStrategy + Send + Sync>,
}

impl StickyCookie {
  pub fn new(
    cookie_name: String,
    inner: Box<dyn LoadBalancingStrategy + Send + Sync>,
    http_only: bool,
    secure: bool,
    same_site: SameSite,
  ) -> StickyCookie {
    let config = Arc::new(StickyCookieConfig {
      cookie_name,
      http_only,
      secure,
      same_site,
    });

    StickyCookie { config, inner }
  }

  fn try_parse_sticky_cookie<'a>(&self, request: &'a Request<Body>) -> Option<Cookie<'a>> {
    let cookie_header = request.headers().get(COOKIE)?;

    cookie_header.to_str().ok()?.split(';').find_map(|cookie_str| {
      let cookie = Cookie::parse(cookie_str).ok()?;
      if cookie.name() == self.config.cookie_name {
        Some(cookie)
      } else {
        None
      }
    })
  }
}

#[async_trait]
impl LoadBalancingStrategy for StickyCookie {
  async fn handle_request(&self, chain: &RequestHandlerChain, lb_context: LoadBalancingContext) -> Response<Body> {
    let index = self.resolve_address_index(&lb_context);

    let authority = &lb_context.pool.addresses[index];

    let context = create_context(index, &lb_context);

    let cookie_missing = true;

    match chain.handle_request(lb_context.request, &context).await {
      Ok(mut response) => {
        if cookie_missing {
          let cookie = Cookie::build(self.config.cookie_name.as_str(), authority)
            .http_only(self.config.http_only)
            .secure(self.config.secure)
            .same_site(self.config.same_site)
            .finish();

          let cookie_val = HeaderValue::from_str(&cookie.to_string()).unwrap();

          match response.headers_mut().entry(SET_COOKIE) {
            Entry::Occupied(mut entry) => {
              entry.append(cookie_val);
            }
            Entry::Vacant(entry) => {
              entry.insert(cookie_val);
            }
          }
        }
        response
      }
      Err(response) => response,
    }

    // self
    //   .try_parse_sticky_cookie(&request)
    //   .and_then(|cookie| lb_context.pool.addresses.iter().position(|a| *a == cookie.value()))
    //   .unwrap_or_else(|| self.inner.resolve_address_index(lb_context))
  }

  fn resolve_address_index(&self, context: &LoadBalancingContext) -> usize {
    self
      .try_parse_sticky_cookie(&context.request)
      .and_then(|cookie| context.pool.addresses.iter().position(|a| *a == cookie.value()))
      .unwrap_or_else(|| self.inner.resolve_address_index(context))
  }
}
