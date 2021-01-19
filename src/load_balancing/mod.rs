use crate::{
  middleware::{RequestHandlerChain, RequestHandlerContext},
  server::BackendPool,
};
use async_trait::async_trait;
use hyper::{Body, Request, Response, Uri};
use std::{net::SocketAddr, sync::Arc};

pub mod ip_hash;
pub mod random;
pub mod round_robin;
pub mod sticky_cookie;

pub struct LoadBalancingContext {
  pub request: Request<Body>,
  pub pool: Arc<BackendPool>,
  pub client_address: SocketAddr,
}

pub async fn handle_request(
  strategy: &Box<dyn LoadBalancingStrategy>,
  chain: &RequestHandlerChain,
  load_balancing_context: LoadBalancingContext,
) -> Response<Body> {
  strategy.handle_request(chain, load_balancing_context).await
}

pub fn create_context(index: usize, context: &LoadBalancingContext) -> RequestHandlerContext {
  let path = context.request.uri().path_and_query().unwrap().clone();

  let backend_uri = Uri::builder()
    .path_and_query(path)
    .scheme("http")
    .authority(context.pool.addresses[index].as_str())
    .build()
    .unwrap();

  RequestHandlerContext {
    client: context.pool.client.clone(),
    backend_uri,
    client_address: context.client_address,
  }
}

#[async_trait]
pub trait LoadBalancingStrategy: Send + Sync + std::fmt::Debug {
  async fn handle_request(&self, chain: &RequestHandlerChain, lb_context: LoadBalancingContext) -> Response<Body> {
    let index = self.resolve_address_index(&lb_context);
    let context = create_context(index, &lb_context);
    // let a = self.analyze_request();
    match chain.handle_request(lb_context.request, &context).await {
      Ok(response) => {
        // modify_response(response, a)
        // ...
        response
      }
      Err(response) => response,
    }
  }

  // fn analyze_request<T>(&self, request: Request<Body>) -> T ;

  fn resolve_address_index(&self, context: &LoadBalancingContext) -> usize;
}
