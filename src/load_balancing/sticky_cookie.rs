use super::{LoadBalancingContext, LoadBalancingStrategy};
use async_trait::async_trait;
use cookie::{Cookie, SameSite};
use hyper::{
  header::{Entry, HeaderValue, COOKIE, SET_COOKIE},
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
  pub inner: Box<dyn LoadBalancingStrategy>,
}

impl StickyCookie {
  pub fn new(
    cookie_name: String,
    inner: Box<dyn LoadBalancingStrategy>,
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

  fn modify_response(
    &self,
    mut response: Response<Body>,
    index: usize,
    lb_context: &LoadBalancingContext,
  ) -> Response<Body> {
    let authority = &lb_context.pool.addresses[index];

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
    response
  }
}

#[async_trait]
impl LoadBalancingStrategy for StickyCookie {
  fn resolve_address_index<'l>(
    &'l self,
    request: &Request<Body>,
    context: &'l LoadBalancingContext,
  ) -> (usize, Box<dyn FnOnce(Response<Body>) -> Response<Body> + Send + 'l>) {
    let index = self
      .try_parse_sticky_cookie(&request)
      .and_then(|cookie| context.pool.addresses.iter().position(|a| *a == cookie.value()));

    if let Some(index) = index {
      (
        index,
        Box::new(move |response| self.modify_response(response, index, context)),
      )
    } else {
      self.inner.resolve_address_index(request, context)
    }
  }
}
