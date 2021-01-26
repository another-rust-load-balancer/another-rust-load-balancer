use super::{compression::split_once, RequestHandler, RequestHandlerChain, RequestHandlerContext};
use async_trait::async_trait;
use hyper::{
  header::{AUTHORIZATION, WWW_AUTHENTICATE},
  Body, HeaderMap, Request, Response, StatusCode,
};
use log::debug;
// TODO 2021-01-26 19:00:52.131554068 ERROR another_rust_load_balancer::middleware - error trying to connect: tcp connect error: Connection refused (os error 111)

#[derive(Debug)]
pub struct Authentication {}

#[async_trait]
impl RequestHandler for Authentication {
  async fn handle_request(
    &self,
    request: Request<Body>,
    next: &RequestHandlerChain,
    context: &RequestHandlerContext<'_>,
  ) -> Result<Response<Body>, Response<Body>> {
    let basic_auth_data = get_basic_auth_data(request.headers());
    debug!("{:#?}", basic_auth_data);
    if let Some(credentials) = basic_auth_data {
      // authenticate (db request)
      debug!("{}", credentials);
      next.handle_request(request, context).await
    //call next handler
    } else {
      let response_builder = Response::builder();
      let response = response_builder
        .header(
          WWW_AUTHENTICATE,
          "Basic realm=\"Another Rust Load Balancer requires authentication\"",
        )
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::from("401 - unauthorized"))
        .unwrap();
      Err(response)
    }
  }
}
#[derive(Debug)]
enum AuthenticationScheme {
  BASIC,
}

impl AuthenticationScheme {
  fn from_str(s: &str) -> Option<AuthenticationScheme> {
    match s {
      "Basic" => Some(AuthenticationScheme::BASIC),
      _ => None,
    }
  }
}

fn get_basic_auth_data(headers: &HeaderMap) -> Option<&str> {
  let auth_data = headers.get(AUTHORIZATION)?.to_str().ok()?;
  let (auth_scheme, credentials) = split_once(auth_data, ' ')?;
  if let Some(AuthenticationScheme::BASIC) = AuthenticationScheme::from_str(auth_scheme) {
    Some(credentials)
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_auth_data_no_auth_header() {
    // given:
    let headers = HeaderMap::new();

    // when:
    let auth_data = get_basic_auth_data(&headers);

    // then:
    assert_eq!(auth_data, None);
  }

  #[test]
  fn test_get_auth_data_wrong_protocol() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Bearer fpKL54jvWmEGVoRdCNjG".parse().unwrap());

    // when:
    let auth_data = get_basic_auth_data(&headers);

    // then:
    assert_eq!(auth_data, None);
  }

  #[test]
  fn test_get_auth_data_basic() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic d2lraTpwZWRpYQ==".parse().unwrap());

    // when:
    let auth_data = get_basic_auth_data(&headers);

    // then:
    assert_ne!(auth_data, None);
  }
}
