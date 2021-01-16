use super::{RequestHandler, RequestHandlerContext};
use crate::load_balancing::sticky_cookie::StickyCookieConfig;
use cookie::{Cookie, SameSite};
use hyper::{
  header::{Entry, HeaderValue, SET_COOKIE},
  Body, Response,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct StickyCookieCompanion {
  pub config: Arc<StickyCookieConfig>,
}

impl StickyCookieCompanion {
  pub fn new(cookie_name: String, http_only: bool, secure: bool, same_site: SameSite) -> StickyCookieCompanion {
    let config = Arc::new(StickyCookieConfig {
      cookie_name,
      http_only,
      secure,
      same_site,
    });
    StickyCookieCompanion { config }
  }
}

impl RequestHandler for StickyCookieCompanion {
  fn modify_response(&self, mut response: Response<Body>, context: &RequestHandlerContext) -> Response<Body> {
    let authority = &context.backend_uri.authority().unwrap().to_string();
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
