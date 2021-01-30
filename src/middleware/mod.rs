use crate::{http_client::StrategyNotifyHttpConnector, server::bad_gateway};
use async_trait::async_trait;
use hyper::{Body, Client, Request, Response, Uri};
use log::error;
use std::net::SocketAddr;

pub mod compression;

pub struct RequestHandlerContext<'l> {
  pub client_address: &'l SocketAddr,
  pub backend_uri: Uri,
  pub client: &'l Client<StrategyNotifyHttpConnector, Body>,
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
    context: &RequestHandlerContext<'_>,
  ) -> Result<Response<Body>, Response<Body>> {
    match self {
      RequestHandlerChain::Entry { handler, next } => handler.handle_request(request, &next, &context).await,
      RequestHandlerChain::Empty => {
        let backend_request = backend_request(context.client_address, &context.backend_uri, request);
        context.client.request(backend_request).await.map_err(|error| {
          error!("{}", error);
          bad_gateway()
        })
      }
    }
  }
}

fn backend_request(client_address: &SocketAddr, backend_uri: &Uri, request: Request<Body>) -> Request<Body> {
  let backend_req_builder = Request::builder().uri(backend_uri);

  request
    .headers()
    .iter()
    .fold(backend_req_builder, |backend_req_builder, (key, val)| {
      backend_req_builder.header(key, val)
    })
    .header("x-forwarded-for", client_address.ip().to_string())
    .method(request.method())
    .body(request.into_body())
    .unwrap()
}

#[async_trait]
pub trait RequestHandler: Send + Sync + std::fmt::Debug {
  async fn handle_request(
    &self,
    request: Request<Body>,
    next: &RequestHandlerChain,
    context: &RequestHandlerContext<'_>,
  ) -> Result<Response<Body>, Response<Body>> {
    let request = self.modify_request(request, context)?;
    let response = next.handle_request(request, context).await?;
    Ok(self.modify_response(response, context))
  }

  fn modify_request(
    &self,
    request: Request<Body>,
    _context: &RequestHandlerContext,
  ) -> Result<Request<Body>, Response<Body>> {
    Ok(request)
  }

  fn modify_response(&self, response: Response<Body>, _context: &RequestHandlerContext) -> Response<Body> {
    response
  }
}
