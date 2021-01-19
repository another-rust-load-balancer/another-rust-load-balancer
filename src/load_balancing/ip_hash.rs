use super::{LoadBalancingContext, LoadBalancingStrategy};
use hyper::{Body, Request, Response};
use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
};

#[derive(Debug)]
pub struct IPHash {}

impl IPHash {
  pub fn new() -> IPHash {
    IPHash {}
  }
}

impl LoadBalancingStrategy for IPHash {
  fn resolve_address_index<'l>(
    &'l self,
    _request: &Request<Body>,
    context: &'l LoadBalancingContext,
  ) -> (usize, Box<dyn FnOnce(Response<Body>) -> Response<Body> + Send + 'l>) {
    let mut hasher = DefaultHasher::new();
    context.client_address.ip().hash(&mut hasher);
    let index = (hasher.finish() % (context.pool.addresses.len() as u64)) as usize;
    (index, Box::new(|it| it))
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use crate::{
    load_balancing::round_robin::RoundRobin,
    middleware::RequestHandlerChain,
    server::{BackendPool, BackendPoolConfig},
  };

  use super::*;

  #[test]
  pub fn ip_hash_strategy_same_ip() {
    let request = Request::builder().body(Body::empty()).unwrap();
    let context = LoadBalancingContext {
      client_address: "127.0.0.1:3000".parse().unwrap(),
      pool: Arc::new(BackendPool::new(
        "whoami.localhost".into(),
        vec!["127.0.0.1:1".into(), "127.0.0.1:2".into()],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      )),
    };
    let strategy = IPHash::new();

    let index = strategy.resolve_address_index(&request, &context).0;
    assert_eq!(strategy.resolve_address_index(&request, &context).0, index);
    assert_eq!(strategy.resolve_address_index(&request, &context).0, index);
    assert_eq!(strategy.resolve_address_index(&request, &context).0, index);
    assert_eq!(strategy.resolve_address_index(&request, &context).0, index);
  }

  #[test]
  pub fn ip_hash_strategy_different_ip() {
    let request_1 = Request::builder().body(Body::empty()).unwrap();
    let context_1 = LoadBalancingContext {
      client_address: "127.0.0.1:3000".parse().unwrap(),
      pool: Arc::new(BackendPool::new(
        "whoami.localhost".into(),
        vec![
          "127.0.0.1:1".into(),
          "127.0.0.1:2".into(),
          "127.0.0.1:3".into(),
          "127.0.0.1:4".into(),
        ],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      )),
    };

    let request_2 = Request::builder().body(Body::empty()).unwrap();
    let context_2 = LoadBalancingContext {
      client_address: "192.168.0.4:3000".parse().unwrap(),
      pool: Arc::new(BackendPool::new(
        "whoami.localhost".into(),
        vec![
          "127.0.0.1:1".into(),
          "127.0.0.1:2".into(),
          "127.0.0.1:3".into(),
          "127.0.0.1:4".into(),
        ],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      )),
    };

    let strategy = IPHash::new();

    assert_ne!(
      strategy.resolve_address_index(&request_1, &context_1).0,
      strategy.resolve_address_index(&request_2, &context_2).0
    );
  }
}
