use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
};

use super::{LBContext, LBStrategy};

#[derive(Debug)]
pub struct IPHash {}

impl IPHash {
  pub fn new() -> IPHash {
    IPHash {}
  }
}

impl LBStrategy for IPHash {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    // finish() does not reset state, so we'll need a new hasher for each request
    let mut hasher = DefaultHasher::new();
    lb_context.client_address.ip().hash(&mut hasher);
    (hasher.finish() % (lb_context.pool.addresses.len() as u64)) as usize
  }
}

#[cfg(test)]
mod tests {
  use hyper::{Body, Request};

  use crate::{
    lb_strategy::round_robin::RoundRobin,
    middleware::RequestHandlerChain,
    server::{BackendPool, BackendPoolConfig},
  };

  use super::*;

  #[test]
  pub fn ip_hash_strategy_same_ip() {
    let lb_context = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2"],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let strategy = IPHash::new();

    let index = strategy.resolve_address_index(&lb_context);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
  }

  #[test]
  pub fn ip_hash_strategy_different_ip() {
    let lb_context_1 = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2", "127.0.0.1:3", "127.0.0.1:4"],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let lb_context_2 = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"192.168.0.4:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2", "127.0.0.1:3", "127.0.0.1:4"],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };

    let strategy = IPHash::new();

    assert_ne!(
      strategy.resolve_address_index(&lb_context_1),
      strategy.resolve_address_index(&lb_context_2)
    );
  }
}
