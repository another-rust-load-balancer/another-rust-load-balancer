use std::sync::Arc;

use cookie::{Cookie, SameSite};
use hyper::{header::COOKIE, Body, Request};

use crate::middleware::sticky_cookie_companion::StickyCookieCompanion;

use super::{LBContext, LBStrategy};
#[derive(Debug)]
pub struct StickyCookieConfig {
  pub cookie_name: &'static str,
  pub secure: bool,
  pub http_only: bool,
  pub same_site: SameSite,
}

#[derive(Debug)]
pub struct StickyCookie {
  pub config: Arc<StickyCookieConfig>,
  pub inner: Box<dyn LBStrategy + Send + Sync>,
}

impl StickyCookie {
  pub fn new(
    cookie_name: &'static str,
    inner: Box<dyn LBStrategy + Send + Sync>,
    http_only: bool,
    secure: bool,
    same_site: SameSite,
  ) -> (StickyCookie, StickyCookieCompanion) {
    let config = Arc::new(StickyCookieConfig {
      cookie_name,
      http_only,
      secure,
      same_site,
    });

    let strategy = StickyCookie {
      config: config.clone(),
      inner,
    };
    let companion = StickyCookieCompanion { config: config.clone() };

    (strategy, companion)
  }

  fn try_parse_sticky_cookie<'a>(&self, request: &'a Request<Body>) -> Option<Cookie<'a>> {
    let cookie_header = request.headers().get(COOKIE)?;

    cookie_header.to_str().ok()?.split(";").find_map(|cookie_str| {
      let cookie = Cookie::parse(cookie_str).ok()?;
      if cookie.name() == self.config.cookie_name {
        Some(cookie)
      } else {
        None
      }
    })
  }
}

impl LBStrategy for StickyCookie {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    self
      .try_parse_sticky_cookie(lb_context.client_request)
      .and_then(|cookie| lb_context.pool.addresses.iter().position(|a| *a == cookie.value()))
      .unwrap_or_else(|| self.inner.resolve_address_index(lb_context))
  }
}
