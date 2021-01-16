use super::{LoadBalancingContext, LoadBalancingStrategy};
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
  fn resolve_address_index(&self, context: &LoadBalancingContext) -> usize {
    // finish() does not reset state, so we'll need a new hasher for each request
    let mut hasher = DefaultHasher::new();
    context.client_address.ip().hash(&mut hasher);
    (hasher.finish() % (context.pool.addresses.len() as u64)) as usize
  }
}

#[cfg(test)]
mod tests {
  use hyper::{Body, Request};

  use crate::{
    load_balancing::round_robin::RoundRobin,
    middleware::RequestHandlerChain,
    server::{BackendPool, BackendPoolConfig},
  };

  use super::*;

  #[test]
  pub fn ip_hash_strategy_same_ip() {
    let context = LoadBalancingContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost".into(),
        vec!["127.0.0.1:1".into(), "127.0.0.1:2".into()],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let strategy = IPHash::new();

    let index = strategy.resolve_address_index(&context);
    assert_eq!(strategy.resolve_address_index(&context), index);
    assert_eq!(strategy.resolve_address_index(&context), index);
    assert_eq!(strategy.resolve_address_index(&context), index);
    assert_eq!(strategy.resolve_address_index(&context), index);
  }

  #[test]
  pub fn ip_hash_strategy_different_ip() {
    let context_1 = LoadBalancingContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
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
      ),
    };
    let context_2 = LoadBalancingContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"192.168.0.4:3000".parse().unwrap(),
      pool: &BackendPool::new(
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
      ),
    };

    let strategy = IPHash::new();

    assert_ne!(
      strategy.resolve_address_index(&context_1),
      strategy.resolve_address_index(&context_2)
    );
  }
}
