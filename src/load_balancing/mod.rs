use crate::{
  middleware::{RequestHandlerChain, RequestHandlerContext},
  server::BackendPool,
};
use async_trait::async_trait;
use hyper::{Body, Request, Response};
use std::{net::SocketAddr, sync::Arc};

pub mod ip_hash;
pub mod random;
pub mod round_robin;
pub mod sticky_cookie;

pub struct LoadBalancingContext {
  pub pool: Arc<BackendPool>,
  pub client_address: SocketAddr,
}

pub async fn handle_request(
  request: Request<Body>,
  strategy: &Box<dyn LoadBalancingStrategy>,
  chain: &RequestHandlerChain,
  context: &LoadBalancingContext,
) -> Response<Body> {
  let (index, response_mapper) = strategy.resolve_address_index(&request, context);
  let context = RequestHandlerContext::new(&request, index, context);
  match chain.handle_request(request, &context).await {
    Ok(response) => response_mapper(response),
    Err(response) => response,
  }
}

#[async_trait]
pub trait LoadBalancingStrategy: Send + Sync + std::fmt::Debug {
  fn resolve_address_index<'l>(
    &'l self,
    request: &Request<Body>,
    context: &'l LoadBalancingContext,
  ) -> (usize, Box<dyn FnOnce(Response<Body>) -> Response<Body> + Send + 'l>);
}
