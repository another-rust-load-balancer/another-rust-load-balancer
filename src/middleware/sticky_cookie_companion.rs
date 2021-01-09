use std::sync::Arc;

use cookie::Cookie;
use hyper::{
  header::{HeaderValue, SET_COOKIE},
  Body, Response,
};

use crate::lb_strategies::StickyCookieStrategyConfig;

use super::{RequestHandler, RequestHandlerContext};

#[derive(Debug)]
pub struct StickyCookieCompanion {
  pub config: Arc<StickyCookieStrategyConfig>,
}

impl RequestHandler for StickyCookieCompanion {
  fn modify_response(&self, mut response: Response<Body>, context: &RequestHandlerContext) -> Response<Body> {
    let authority = &context.backend_uri.authority().unwrap().to_string();
    let cookie = Cookie::build(self.config.cookie_name, authority)
      .http_only(self.config.http_only)
      .secure(self.config.secure)
      .same_site(self.config.same_site)
      .finish();

    response
      .headers_mut()
      .insert(SET_COOKIE, HeaderValue::from_str(&cookie.to_string()).unwrap());

    response
  }
}
