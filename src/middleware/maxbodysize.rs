use super::{super::error_response, Context, Middleware};
use async_trait::async_trait;
use hyper::{header::CONTENT_LENGTH, Body, HeaderMap, Request, Response};

#[derive(Debug)]
pub struct MaxBodySize {
  pub(crate) limit: i64,
}

#[async_trait]
impl Middleware for MaxBodySize {
  fn modify_request(&self, request: Request<Body>, _context: &Context) -> Result<Request<Body>, Response<Body>> {
    match get_content_length(request.headers()) {
      Some(length) if length > self.limit => Err(error_response::request_entity_to_large()),
      _ => Ok(request),
    }
  }
}

fn get_content_length(headers: &HeaderMap) -> Option<i64> {
  headers.get(CONTENT_LENGTH)?.to_str().ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_content_length_no_headers() {
    // given:
    let headers = HeaderMap::new();

    // when:
    let actual = get_content_length(&headers);

    // then:
    assert_eq!(actual, None);
  }

  #[test]
  fn test_get_content_length_non_integer() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_LENGTH, "unknown".parse().unwrap());

    // when:
    let actual = get_content_length(&headers);

    // then:
    assert_eq!(actual, None);
  }

  #[test]
  fn test_get_content_length_integer() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_LENGTH, "256".parse().unwrap());

    // when:
    let actual = get_content_length(&headers);

    // then:
    assert_eq!(actual, Some(256));
  }
}
