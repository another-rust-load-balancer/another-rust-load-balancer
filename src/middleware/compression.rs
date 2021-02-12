use crate::utils::split_once;

use super::{Context, Middleware, MiddlewareChain};
use async_compression::tokio::bufread::{BrotliEncoder, DeflateEncoder, GzipEncoder};
use async_trait::async_trait;
use futures::TryStreamExt;
use hyper::{
  header::{HeaderValue, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH},
  Body, HeaderMap, Request, Response,
};
use std::{
  fmt::Display,
  io::{self, ErrorKind},
};
use tokio::io::AsyncRead;
use tokio_util::{
  codec::{BytesCodec, FramedRead},
  io::StreamReader,
};
use Encoding::{BROTLI, DEFLATE, GZIP};

#[derive(Debug)]
pub struct Compression;

#[async_trait]
impl Middleware for Compression {
  async fn forward_request(
    &self,
    request: Request<Body>,
    chain: &MiddlewareChain,
    context: &Context<'_>,
  ) -> Response<Body> {
    let encoding = get_preferred_encoding(request.headers());
    let response = chain.forward_request(request, context).await;
    if let Some(encoding) = encoding.filter(|_| !response.headers().contains_key(CONTENT_ENCODING)) {
      self.compress_response(response, &encoding)
    } else {
      response
    }
  }
}

impl Compression {
  fn compress_response(&self, response: Response<Body>, encoding: &Encoding) -> Response<Body> {
    let (parts, body) = response.into_parts();

    let stream = StreamReader::new(body.map_err(|error| io::Error::new(ErrorKind::Other, error)));

    let body = match encoding {
      BROTLI => to_body(BrotliEncoder::new(stream)),
      DEFLATE => to_body(DeflateEncoder::new(stream)),
      GZIP => to_body(GzipEncoder::new(stream)),
    };
    fn to_body<S>(stream: S) -> hyper::Body
    where
      S: AsyncRead + Send + 'static,
    {
      Body::wrap_stream(FramedRead::new(stream, BytesCodec::new()))
    }

    let mut response = Response::from_parts(parts, body);
    let headers = response.headers_mut();
    headers.insert(CONTENT_ENCODING, encoding.into());
    headers.remove(CONTENT_LENGTH);
    response
  }
}

#[derive(Debug, PartialEq)]
enum Encoding {
  BROTLI,
  DEFLATE,
  GZIP,
}

impl Encoding {
  fn to_str(&self) -> &'static str {
    match self {
      BROTLI => "br",
      DEFLATE => "deflate",
      GZIP => "gzip",
    }
  }

  fn from_str(s: &str) -> Option<Encoding> {
    match s {
      "br" => Some(BROTLI),
      "deflate" => Some(DEFLATE),
      "gzip" => Some(GZIP),
      _ => None,
    }
  }
}

impl From<&Encoding> for HeaderValue {
  fn from(encoding: &Encoding) -> Self {
    HeaderValue::from_static(encoding.to_str())
  }
}

impl Display for Encoding {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.to_str())
  }
}

/// Parses the Header `Accept-Encoding` (as definied in [RFC 7231, section 5.3.4: Accept-Encoding](https://tools.ietf.org/html/rfc7231#section-5.3.4))
/// in the specified `HeaderMap` returning the preferred supported `Encoding` if one exists.
///
/// Do determine which `Encoding` is preferred by the client, Quality Values (as defined in [RFC 7231, section 5.3.1: Quality Values](https://tools.ietf.org/html/rfc7231#section-5.3.1)) are used.
///
/// The special encoding `*` is not currently supported.
fn get_preferred_encoding(headers: &HeaderMap) -> Option<Encoding> {
  headers
    .get(ACCEPT_ENCODING)?
    .to_str()
    .ok()?
    .split(',')
    .map(|it| it.trim())
    .filter_map(parse_encoding_and_qvalue)
    .rev() // max_by_key returns last of equal elements, so reverse to get the first instead
    .max_by_key(|(_encoding, qvalue)| *qvalue)
    .map(|(encoding, _qvalue)| encoding)
}

fn parse_encoding_and_qvalue(encoding_and_qvalue: &str) -> Option<(Encoding, u32)> {
  let (encoding, qvalue) = split_once(encoding_and_qvalue, ';').unwrap_or((encoding_and_qvalue, &"q=1"));
  let encoding = Encoding::from_str(encoding)?;
  let qvalue = parse_qvalue(qvalue)?;
  Some((encoding, qvalue))
}

/// Parses the Quality Value (as defined in [RFC 7231, section 5.3.1: Quality Values](https://tools.ietf.org/html/rfc7231#section-5.3.1)) as an `u32`.
///
/// Using `u32` instead of `f32` is possible because the precision is limited to 3 digits after the decimal point.
/// `u32` has the advantage that it implements `Ord` and not just `PartialOrd` which is important for methods like `max_by_key`.
fn parse_qvalue(qvalue: &str) -> Option<u32> {
  let qvalue = qvalue.strip_prefix("q=")?;
  if qvalue == "1" {
    Some(1000)
  } else {
    let qvalue = qvalue.strip_prefix("0.").filter(|digits| digits.len() <= 3)?;
    format!("{:0<3}", qvalue).parse().ok().filter(|qvalue| *qvalue != 0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_preferred_encoding_no_headers() {
    // given:
    let headers = HeaderMap::new();

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, None);
  }

  #[test]
  fn test_get_preferred_encoding_unknown_encoding() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "unknown".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, None);
  }

  #[test]
  fn test_get_preferred_encoding_brotli() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "br".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(BROTLI));
  }

  #[test]
  fn test_get_preferred_encoding_deflate() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(DEFLATE));
  }

  #[test]
  fn test_get_preferred_encoding_gzip() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(GZIP));
  }

  #[test]
  fn test_get_preferred_encoding_deflate_and_gzip() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate, gzip".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(DEFLATE));
  }

  #[test]
  fn test_get_preferred_encoding_deflate_and_gzip_qvalues() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate;q=0.9, gzip;q=1".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(GZIP));
  }

  #[test]
  fn test_get_preferred_encoding_deflate_and_gzip_qvalues_with_different_decimal_places() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate;q=0.75, gzip;q=0.8".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(GZIP));
  }

  #[test]
  fn test_get_preferred_encoding_deflate_zero_qvalue() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate;q=0".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, None);
  }

  #[test]
  fn test_get_preferred_encoding_deflate_zero_qvalue_with_decimals() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate;q=0.000".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, None);
  }

  #[test]
  fn test_get_preferred_encoding_deflate_zero_qvalue_gzip_non_zero_qvalue() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "deflate;q=0, gzip;q=0.5".parse().unwrap());

    // when:
    let actual = get_preferred_encoding(&headers);

    // then:
    assert_eq!(actual, Some(GZIP));
  }
}
