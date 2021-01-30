use super::{Context, LoadBalancingStrategy, RequestForwarder};
use hyper::{Body, Request};
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
  fn select_backend<'l>(&'l self, _request: &Request<Body>, context: &'l Context) -> RequestForwarder {
    let mut hasher = DefaultHasher::new();
    context.client_address.ip().hash(&mut hasher);
    let index = (hasher.finish() % (context.backend_addresses.len() as u64)) as usize;
    let address = &context.backend_addresses[index];
    RequestForwarder::new(address)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn ip_hash_strategy_same_ip() {
    let request = Request::builder().body(Body::empty()).unwrap();
    let context = Context {
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      backend_addresses: &mut ["127.0.0.1:1".into(), "127.0.0.1:2".into()],
    };
    let strategy = IPHash::new();

    let address = strategy.select_backend(&request, &context).backend_address;
    assert_eq!(strategy.select_backend(&request, &context).backend_address, address);
    assert_eq!(strategy.select_backend(&request, &context).backend_address, address);
    assert_eq!(strategy.select_backend(&request, &context).backend_address, address);
    assert_eq!(strategy.select_backend(&request, &context).backend_address, address);
  }

  #[test]
  pub fn ip_hash_strategy_different_ip() {
    let request_1 = Request::builder().body(Body::empty()).unwrap();
    let context_1 = Context {
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      backend_addresses: &mut [
        "127.0.0.1:1".into(),
        "127.0.0.1:2".into(),
        "127.0.0.1:3".into(),
        "127.0.0.1:4".into(),
      ],
    };

    let request_2 = Request::builder().body(Body::empty()).unwrap();
    let context_2 = Context {
      client_address: &"192.168.0.4:3000".parse().unwrap(),
      backend_addresses: &mut [
        "127.0.0.1:1".into(),
        "127.0.0.1:2".into(),
        "127.0.0.1:3".into(),
        "127.0.0.1:4".into(),
      ],
    };

    let strategy = IPHash::new();

    assert_ne!(
      strategy.select_backend(&request_1, &context_1).backend_address,
      strategy.select_backend(&request_2, &context_2).backend_address
    );
  }
}
