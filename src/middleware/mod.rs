use crate::{error_response::handle_bad_gateway, http_client::StrategyNotifyHttpConnector, server::Scheme};
use async_trait::async_trait;
use hyper::{header::HeaderValue, Body, Client, Request, Response, Uri};
use std::net::SocketAddr;

pub mod authentication;
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
    let request = self.modify_request(request, context).await?;
    let response = chain.forward_request(request, context).await?;
    Ok(self.modify_response(response, context))
  }

  async fn modify_request(
    &self,
    request: Request<Body>,
    _context: &Context<'_>,
  ) -> Result<Request<Body>, Response<Body>> {
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

  let mut builder = request
    .headers()
    .iter()
    .fold(builder, |builder, (key, val)| builder.header(key, val))
    .header(
      "x-forwarded-for",
      forwarded_for_header(
        request.headers().get("x-forwarded-for"),
        context.client_address.ip().to_string(),
      ),
    )
    .header("x-real-ip", context.client_address.ip().to_string())
    .header(
      "x-forwarded-port",
      match context.client_scheme {
        Scheme::HTTP => "80",
        Scheme::HTTPS => "443",
      },
    )
    .header("x-forwarded-proto", context.client_scheme.to_string())
    .method(request.method());

  builder = if let Ok(hostname) = gethostname().into_string() {
    builder.header("x-forwarded-server", hostname)
  } else {
    builder
  };

  builder.body(request.into_body()).unwrap()
}

// According to https://docs.oracle.com/en-us/iaas/Content/Balance/Reference/httpheaders.htm
fn forwarded_for_header(existing_forwarded_for: Option<&HeaderValue>, client_ip: String) -> String {
  match existing_forwarded_for {
    Some(existing_forwarded_for) => {
      let mut forwarded_for = existing_forwarded_for.to_str().unwrap_or("").to_owned();
      forwarded_for.push_str(&format!(", {}", &client_ip));
      forwarded_for
    }
    None => client_ip,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_build_forwarded_for_header_empty() {
    let forwarded_for_header = forwarded_for_header(None, "127.0.0.1".into());

    assert_eq!(forwarded_for_header, "127.0.0.1");
  }

  #[test]
  fn test_build_forwarded_for_header_existing() {
    let forwarded_for_header = forwarded_for_header(Some(&HeaderValue::from_static("127.0.0.2")), "127.0.0.1".into());

    assert_eq!(forwarded_for_header, "127.0.0.2, 127.0.0.1");
  }
}
