use crate::{
  error_response::handle_bad_gateway, http_client::StrategyNotifyHttpConnector, server::Scheme, utils::unwrap_result,
};
use async_trait::async_trait;
use gethostname::gethostname;
use hyper::{header::HeaderValue, Body, Client, Request, Response, Uri};
use std::net::SocketAddr;

pub mod authentication;
pub mod compression;
pub mod custom_error_pages;
pub mod https_redirector;
pub mod maxbodysize;
pub mod rate_limiter;

/// A trait for implementing middlewares, see
/// [`forward_request`](Middleware::forward_request) for more details.
#[async_trait]
pub trait Middleware: Send + Sync + std::fmt::Debug {
  /// Forward the `request` via [`MiddlewareChain::forward_request`],
  /// potentially modifying the request or response along the way. The request
  /// can also be answered early (from cache or in cases like missing
  /// authentication) in which case the supplied `chain` is not used.
  ///
  /// The default implementation calls [`modify_request`][], forwards the
  /// modified request via [`MiddlewareChain::forward_request`] and then calls
  /// [`modify_response`][] on the received response. Simple implementations
  /// should prefer to just implement [`modify_request`][] or
  /// [`modify_response`][] if possible.
  ///
  /// [`modify_request`]: Middleware::modify_request
  /// [`modify_response`]: Middleware::modify_response
  async fn forward_request(
    &self,
    request: Request<Body>,
    chain: &MiddlewareChain,
    context: &Context<'_>,
  ) -> Response<Body> {
    match self.modify_request(request, context).await {
      Ok(request) => {
        let response = chain.forward_request(request, context).await;
        self.modify_response(response, context).await
      }
      Err(response) => response,
    }
  }

  /// Transforms the `request` before it is forwarded to later middlewares and
  /// then the backend server or returns an early response (from cache or in
  /// cases like missing authentication).
  ///
  /// If an early response is returned then
  /// [`modify_response`](Middleware::modify_response) of this middleware is not
  /// called.
  ///
  /// This method serves only as a convenience to implement simple middlewares.
  /// If [`forward_request`](Middleware::forward_request) is implemented, this
  /// function is never called.
  ///
  /// The default implementation just returns the original `request`.
  async fn modify_request(
    &self,
    request: Request<Body>,
    _context: &Context<'_>,
  ) -> Result<Request<Body>, Response<Body>> {
    Ok(request)
  }

  /// Transforms the response before it is returned to the calling client.
  ///
  /// This function is not called for early responses of the same middleware,
  /// but it is called for early responses of middlewares further down the
  /// [`MiddlewareChain`].
  ///
  /// This method serves only as a convenience to implement simple middlewares.
  /// If [`forward_request`](Middleware::forward_request) is implemented, this
  /// function is never called.
  ///
  /// The default implementation just returns the original `response`.
  async fn modify_response(&self, response: Response<Body>, _context: &Context<'_>) -> Response<Body> {
    response
  }
}

pub struct Context<'l> {
  pub client_scheme: &'l Scheme,
  pub client_address: &'l SocketAddr,
  pub backend_uri: Uri,
  pub client: &'l Client<StrategyNotifyHttpConnector, Body>,
}

/// A singly linked list of [`Middleware`]s.
///
/// This list is used to call all middlewares of a
/// [`BackendPool`](crate::server::BackendPool) in order and forward the request
/// to the backend server once the chain is empty. This is implemented in
/// [`forward_request`](MiddlewareChain::forward_request).
#[derive(Debug)]
pub enum MiddlewareChain {
  Empty,
  Entry {
    middleware: Box<dyn Middleware>,
    chain: Box<MiddlewareChain>,
  },
}

impl MiddlewareChain {
  /// If this chain is not empty this function calls
  /// [`forward_request`](Middleware::forward_request) on the first middleware,
  /// passing it the tail of this chain as an argument to be called recursively.
  ///
  /// Once this chain is empty this function does the final request
  /// transformation, setting all appropriate forwarding headers (like
  /// `x-forwarded-for`) and sends it to the backend server, returning the
  /// response.
  pub async fn forward_request(&self, request: Request<Body>, context: &Context<'_>) -> Response<Body> {
    match self {
      MiddlewareChain::Entry { middleware, chain } => middleware.forward_request(request, &chain, &context).await,
      MiddlewareChain::Empty => {
        let backend_request = backend_request(request, context);
        unwrap_result(
          context
            .client
            .request(backend_request)
            .await
            .map_err(handle_bad_gateway),
        )
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
