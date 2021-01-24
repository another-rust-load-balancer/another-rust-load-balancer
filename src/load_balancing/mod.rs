use crate::{
  http_client::StrategyNotifyHttpConnector,
  middleware::{RequestHandlerChain, RequestHandlerContext},
};
use async_trait::async_trait;
use hyper::{Body, Client, Request, Response, Uri};
use std::{convert::identity, net::SocketAddr};

pub mod ip_hash;
pub mod least_connection;
pub mod random;
pub mod round_robin;
pub mod sticky_cookie;

#[async_trait]
pub trait LoadBalancingStrategy: Send + Sync + std::fmt::Debug {
  /// called when a new TCP connection to a backend server opened
  fn on_tcp_open(&self, _remote: &Uri) {}

  /// called when an existing backend TCP connection is closed
  fn on_tcp_close(&self, _remote: &Uri) {}
  fn select_backend<'l>(&'l self, request: &Request<Body>, context: &'l LoadBalancingContext) -> RequestForwarder;
}

pub struct LoadBalancingContext<'l> {
  pub client_address: &'l SocketAddr,
  pub backend_addresses: &'l [String],
}

pub struct RequestForwarder<'l> {
  address: &'l str,
  response_mapper: Box<dyn Fn(Response<Body>) -> Response<Body> + Send + Sync + 'l>,
}

impl<'l> RequestForwarder<'l> {
  fn new(address: &str) -> RequestForwarder {
    RequestForwarder::new_with_response_mapper(address, identity)
  }

  fn new_with_response_mapper<'n, F>(address: &'n str, response_mapper: F) -> RequestForwarder<'n>
  where
    F: Fn(Response<Body>) -> Response<Body> + Send + Sync + 'n,
  {
    RequestForwarder {
      address,
      response_mapper: Box::new(response_mapper),
    }
  }

  fn map_response<F>(self, response_mapper: F) -> RequestForwarder<'l>
  where
    F: Fn(Response<Body>) -> Response<Body> + Send + Sync + 'l,
  {
    RequestForwarder {
      address: self.address,
      response_mapper: Box::new(move |response| response_mapper((self.response_mapper)(response))),
    }
  }

  pub async fn forward_request(
    &self,
    request: Request<Body>,
    chain: &RequestHandlerChain,
    context: &LoadBalancingContext<'_>,
    client: &Client<StrategyNotifyHttpConnector, Body>,
  ) -> Response<Body> {
    let context = RequestHandlerContext {
      client_address: context.client_address,
      backend_uri: self.uri(&request),
      client,
    };
    match chain.handle_request(request, &context).await {
      Ok(response) => (self.response_mapper)(response),
      Err(response) => response,
    }
  }

  fn uri(&self, request: &Request<Body>) -> Uri {
    let path = request.uri().path_and_query().unwrap().clone();
    Uri::builder()
      .scheme("http")
      .authority(self.address)
      .path_and_query(path)
      .build()
      .unwrap()
  }
}
