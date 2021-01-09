use super::{RequestHandler, RequestHandlerChain, RequestHandlerContext};
use async_compression::stream::GzipEncoder;
use async_trait::async_trait;
use futures::TryStreamExt;
use hyper::{header::HeaderValue, Body, Request, Response};
use std::io;
use std::io::ErrorKind;

#[derive(Debug)]
pub struct Compression {}

enum CompressionAlgorithm {
  GZIP,
}

#[async_trait]
impl RequestHandler for Compression {
  async fn handle_request(
    &self,
    request: Request<Body>,
    next: &RequestHandlerChain,
    context: &RequestHandlerContext,
  ) -> Result<Response<Body>, Response<Body>> {
    let header = request.headers().get("Accept-Encoding");
    let compress = header.map_or(false, |value| {
      value.to_str().map_or(false, |string| string.contains(&"gzip"))
    });
    next.handle_request(request, context).await.map(|response| {
      if compress {
        self.compress_response(response, CompressionAlgorithm::GZIP)
      } else {
        response
      }
    })
  }
}
impl Compression {
  fn compress_response(&self, response: Response<Body>, algorithm: CompressionAlgorithm) -> Response<Body> {
    let (parts, body) = response.into_parts();

    let stream = body
      .map_ok(|chunk| bytes::Bytes::from(chunk.to_vec()))
      .map_err(|error| io::Error::new(ErrorKind::Other, error));
    let compressed_stream = match algorithm {
      CompressionAlgorithm::GZIP => GzipEncoder::new(stream),
    };
    let body = Body::wrap_stream(compressed_stream.map_ok(|chunk| hyper::body::Bytes::from(chunk.to_vec())));

    let mut response = Response::from_parts(parts, body);
    let headers = response.headers_mut();
    headers.insert("Content-Encoding", HeaderValue::from_static("gzip"));
    headers.remove("Content-Length");
    response
  }
}
