use super::{Context, Middleware};
use http_auth_basic::Credentials;
use hyper::{
  header::{AUTHORIZATION, WWW_AUTHENTICATE},
  Body, HeaderMap, Request, Response, StatusCode,
};
use std::{
  fs::File,
  io::{BufRead, BufReader},
};

#[derive(Debug)]
pub struct Authentication {}

impl Middleware for Authentication {
  fn modify_request(&self, request: Request<Body>, _context: &Context) -> Result<Request<Body>, Response<Body>> {
    if user_authentication(request.headers()).is_some() {
      Ok(request)
    } else {
      Err(response_unauthorized())
    }
  }
}

fn user_authentication(headers: &HeaderMap) -> Option<()> {
  let auth_data = headers.get(AUTHORIZATION)?.to_str().ok()?;
  let credentials = Credentials::from_header(auth_data.to_string()).ok()?;
  check_user_credentials(&credentials.user_id, &credentials.password)
}

fn check_user_credentials(user: &str, password: &str) -> Option<()> {
  let file = File::open("src/middleware/properties.txt").ok()?;
  let reader = BufReader::new(file);
  for line in reader.lines() {
    if let Ok(l) = line {
      let credentials = format!("{}={}", user, password);
      if credentials == l {
        return Some(());
      }
    }
  }
  None
}

fn response_unauthorized() -> Response<Body> {
  let response_builder = Response::builder();
  let response = response_builder
    .header(
      WWW_AUTHENTICATE,
      "Basic realm=\"Another Rust Load Balancer requires authentication\"",
    )
    .status(StatusCode::UNAUTHORIZED)
    .body(Body::from("401 - unauthorized"))
    .unwrap();
  response
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_user_authentication_no_header() {
    // given:
    let headers = HeaderMap::new();

    // when:
    let auth_data = user_authentication(&headers);

    // then:
    assert!(auth_data.is_none());
  }

  #[test]
  fn test_user_authentication_wrong_protocol() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Bearer fpKL54jvWmEGVoRdCNjG".parse().unwrap());

    // when:
    let auth_data = user_authentication(&headers);

    // then:
    assert!(auth_data.is_none());
  }

  #[test]
  fn test_user_authentication_basic_authorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());

    // when:
    let auth_data = user_authentication(&headers);

    // then:
    assert!(auth_data.is_some());
  }

  #[test]
  fn test_user_authentication_basic_unauthorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbwP==".parse().unwrap());

    // when:
    let auth_data = user_authentication(&headers);

    // then:
    assert!(auth_data.is_none());
  }
}
