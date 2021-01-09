use crate::server::bad_gateway;
use async_compression::stream::GzipEncoder;
use async_trait::async_trait;
use futures::TryStreamExt;
use hyper::{client::HttpConnector, header::HeaderValue, Body, Client, Request, Response, Uri};
use log::error;
use std::{
  io::{self, ErrorKind},
  net::SocketAddr,
  sync::Arc,
};

pub struct RequestHandlerContext {
  pub client_address: SocketAddr,
  pub backend_uri: Uri,
  pub client: Arc<Client<HttpConnector, Body>>,
}

#[derive(Debug)]
pub enum RequestHandlerChain {
  Empty,
  Entry {
    handler: Box<dyn RequestHandler>,
    next: Box<RequestHandlerChain>,
  },
}

impl RequestHandlerChain {
  pub async fn handle_request(
    &self,
    request: Request<Body>,
    context: &RequestHandlerContext,
  ) -> Result<Response<Body>, Response<Body>> {
    match self {
      RequestHandlerChain::Entry { handler, next } => handler.handle_request(request, &next, &context).await,
      RequestHandlerChain::Empty => {
        let backend_request = backend_request(&context.client_address, &context.backend_uri, request);
        context.client.request(backend_request).await.map_err(|error| {
          error!("{}", error);
          bad_gateway()
        })
      }
    }
  }
}

fn backend_request(client_address: &SocketAddr, backend_uri: &Uri, client_request: Request<Body>) -> Request<Body> {
  let backend_req_builder = Request::builder().uri(backend_uri);

  client_request
    .headers()
    .iter()
    .fold(backend_req_builder, |backend_req_builder, (key, val)| {
      backend_req_builder.header(key, val)
    })
    .header("x-forwarded-for", client_address.ip().to_string())
    .method(client_request.method())
    .body(client_request.into_body())
    .unwrap()
}

#[async_trait]
pub trait RequestHandler: Send + Sync + std::fmt::Debug {
  async fn handle_request(
    &self,
    request: Request<Body>,
    next: &RequestHandlerChain,
    context: &RequestHandlerContext,
  ) -> Result<Response<Body>, Response<Body>> {
    match self.modify_client_request(request, context) {
      Ok(request) => next
        .handle_request(request, context)
        .await
        .map(|response| self.modify_response(response, context)),
      Err(response) => Err(response),
    }
  }

  fn modify_client_request(
    &self,
    client_request: Request<Body>,
    _context: &RequestHandlerContext,
  ) -> Result<Request<Body>, Response<Body>> {
    Ok(client_request)
  }

  fn modify_response(&self, response: Response<Body>, _context: &RequestHandlerContext) -> Response<Body> {
    response
  }
}

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
