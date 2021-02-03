use crate::{error_response::handle_bad_gateway, http_client::StrategyNotifyHttpConnector, server::Scheme};
use async_trait::async_trait;
use gethostname::gethostname;
use hyper::{Body, Client, Request, Response, Uri};
use std::net::SocketAddr;

pub mod compression;
pub mod https_redirector;
pub mod maxbodysize;

#[async_trait]
pub trait Middleware: Send + Sync + std::fmt::Debug {
  async fn forward_request(
    &self,
    request: Request<Body>,
    chain: &MiddlewareChain,
    context: &Context<'_>,
  ) -> Result<Response<Body>, Response<Body>> {
    let request = self.modify_request(request, context)?;
    let response = chain.forward_request(request, context).await?;
    Ok(self.modify_response(response, context))
  }

  fn modify_request(&self, request: Request<Body>, _context: &Context) -> Result<Request<Body>, Response<Body>> {
    Ok(request)
  }

  fn modify_response(&self, response: Response<Body>, _context: &Context) -> Response<Body> {
    response
  }
}

pub struct Context<'l> {
  pub client_scheme: &'l Scheme,
  pub client_address: &'l SocketAddr,
  pub backend_uri: Uri,
  pub client: &'l Client<StrategyNotifyHttpConnector, Body>,
}

#[derive(Debug)]
pub enum MiddlewareChain {
  Empty,
  Entry {
    middleware: Box<dyn Middleware>,
    chain: Box<MiddlewareChain>,
  },
}

impl MiddlewareChain {
  pub async fn forward_request(
    &self,
    request: Request<Body>,
    context: &Context<'_>,
  ) -> Result<Response<Body>, Response<Body>> {
    match self {
      MiddlewareChain::Entry { middleware, chain } => middleware.forward_request(request, &chain, &context).await,
      MiddlewareChain::Empty => {
        let backend_request = backend_request(request, context);
        context
          .client
          .request(backend_request)
          .await
          .map_err(handle_bad_gateway)
      }
    }
  }
}

fn backend_request(request: Request<Body>, context: &Context) -> Request<Body> {
  let builder = Request::builder().uri(&context.backend_uri);
  let hostname = gethostname().into_string().ok();

  let mut builder = request
    .headers()
    .iter()
    .fold(builder, |builder, (key, val)| builder.header(key, val))
    .header("x-forwarded-for", context.client_address.ip().to_string())
    .header(
      "x-forwarded-port",
      match context.client_scheme {
        Scheme::HTTP => "80",
        Scheme::HTTPS => "443",
      },
    )
    .header("x-forwarded-proto", context.client_scheme.to_string())
    .method(request.method());

  builder = if let Some(hostname) = hostname {
    builder.header("x-forwarded-server", hostname)
  } else {
    builder
  };

  builder.body(request.into_body()).unwrap()
}
