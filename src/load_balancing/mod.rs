use crate::{
  middleware::{RequestHandlerChain, RequestHandlerContext},
  server::BackendPool,
};
use async_trait::async_trait;
use hyper::{Body, Request, Response};
use std::{convert::identity, net::SocketAddr, sync::Arc};

pub mod ip_hash;
pub mod random;
pub mod round_robin;
pub mod sticky_cookie;

pub struct LoadBalancingContext {
  pub pool: Arc<BackendPool>,
  pub client_address: SocketAddr,
}

pub struct LoadBalanceTarget<'l> {
  index: usize,
  response_mapper: Box<dyn FnOnce(Response<Body>) -> Response<Body> + Send + 'l>,
}

impl<'l> LoadBalanceTarget<'l> {
  fn new(index: usize) -> LoadBalanceTarget<'static> {
    LoadBalanceTarget::new_with_response_mapping(index, identity)
  }

  fn new_with_response_mapping<F>(index: usize, response_mapper: F) -> LoadBalanceTarget<'l>
  where
    F: FnOnce(Response<Body>) -> Response<Body> + Send + 'l,
  {
    LoadBalanceTarget {
      index,
      response_mapper: Box::new(response_mapper),
    }
  }

  fn map_response<F>(self, response_mapper: F) -> LoadBalanceTarget<'l>
  where
    F: FnOnce(Response<Body>) -> Response<Body> + Send + 'l,
  {
    LoadBalanceTarget {
      index: self.index,
      response_mapper: Box::new(move |response| response_mapper((self.response_mapper)(response))),
    }
  }
}

pub async fn handle_request(
  request: Request<Body>,
  strategy: &Box<dyn LoadBalancingStrategy>,
  chain: &RequestHandlerChain,
  context: &LoadBalancingContext,
) -> Response<Body> {
  let target = strategy.resolve_address_index(&request, context);
  let context = RequestHandlerContext::new(&request, target.index, context);
  match chain.handle_request(request, &context).await {
    Ok(response) => (target.response_mapper)(response),
    Err(response) => response,
  }
}

#[async_trait]
pub trait LoadBalancingStrategy: Send + Sync + std::fmt::Debug {
  fn resolve_address_index<'l>(
    &'l self,
    request: &Request<Body>,
    context: &'l LoadBalancingContext,
  ) -> LoadBalanceTarget;
}
