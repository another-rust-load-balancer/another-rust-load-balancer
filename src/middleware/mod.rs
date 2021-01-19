use crate::{load_balancing::LoadBalancingContext, server::bad_gateway};
use async_trait::async_trait;
use hyper::{client::HttpConnector, Body, Client, Request, Response, Uri};
use log::error;
use std::{net::SocketAddr, sync::Arc};

pub mod compression;
pub mod sticky_cookie_companion;

pub struct RequestHandlerContext {
  pub client_address: SocketAddr,
  pub backend_uri: Uri,
  pub client: Arc<Client<HttpConnector, Body>>,
}

impl RequestHandlerContext {
  pub fn new(request: &Request<Body>, index: usize, context: &LoadBalancingContext) -> RequestHandlerContext {
    let authority = context.pool.addresses[index].as_str();
    let backend_uri = backend_uri(request, authority);
    RequestHandlerContext {
      client: context.pool.client.clone(),
      backend_uri,
      client_address: context.client_address,
    }
  }
}

fn backend_uri(request: &Request<Body>, authority: &str) -> Uri {
  let path = request.uri().path_and_query().unwrap().clone();
  Uri::builder()
    .path_and_query(path)
    .scheme("http")
    .authority(authority)
    .build()
    .unwrap()
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
