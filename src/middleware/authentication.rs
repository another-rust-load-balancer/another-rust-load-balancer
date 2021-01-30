use super::{RequestHandler, RequestHandlerContext};
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

impl RequestHandler for Authentication {
  fn modify_client_request(
    &self,
    client_request: Request<Body>,
    _context: &RequestHandlerContext,
  ) -> Result<Request<Body>, Response<Body>> {
    let basic_auth_data = get_basic_auth_data(client_request.headers());
    if let Some(credentials) = basic_auth_data {
      if authenticate_user(&credentials) {
        Ok(client_request)
      } else {
        Err(response_unauthorized())
      }
    } else {
      Err(response_unauthorized())
    }
  }
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

fn authenticate_user(credentials: &str) -> bool {
  let file = File::open("src/middleware/properties.txt");
  match file {
    Ok(f) => {
      let reader = BufReader::new(f);
      for line in reader.lines() {
        if let Ok(l) = line {
          if credentials.to_string() == l {
            return true;
          }
        }
      }
      false
    }
    _ => false,
  }
}

fn get_basic_auth_data(headers: &HeaderMap) -> Option<String> {
  let auth_data = headers.get(AUTHORIZATION)?.to_str().ok()?;
  let credentials = Credentials::from_header(auth_data.to_string());
  if let Ok(credentials) = credentials {
    Some(format!("{}={}", credentials.user_id, credentials.password).to_string())
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

  #[test]
  fn test_authenticate_user_authorized() {
    let credential_string = "tyrion=foo";
    let auth = authenticate_user(&credential_string);
    let mut result = false;
    if auth {
      result = true;
    }
    assert_eq!(true, result)
  }

  #[test]
  fn test_authenticate_user_unauthorized() {
    let credential_string = "tyrion=bar";
    let auth = authenticate_user(&credential_string);
    let mut result = false;
    if auth {
      result = true;
    }
    assert_eq!(false, result)
  }
}
