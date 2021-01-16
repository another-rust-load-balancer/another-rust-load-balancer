use super::{LoadBalancingContext, LoadBalancingStrategy};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct RoundRobin {
  rrc: Arc<Mutex<u32>>,
}

impl RoundRobin {
  pub fn new() -> RoundRobin {
    RoundRobin {
      rrc: Arc::new(Mutex::new(0)),
    }
  }
}

impl LoadBalancingStrategy for RoundRobin {
  fn resolve_address_index(&self, lb_context: &LoadBalancingContext) -> usize {
    let mut rrchandle = self.rrc.lock().unwrap();
    *rrchandle = (*rrchandle + 1) % lb_context.pool.addresses.len() as u32;
    *rrchandle as usize
  }
}

#[cfg(test)]
mod tests {
  use hyper::{Body, Request};

  use crate::{
    middleware::RequestHandlerChain,
    server::{BackendPool, BackendPoolConfig},
  };

  use super::*;

  #[test]
  pub fn round_robin_strategy_single_address() {
    let lb_context = LoadBalancingContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost".into(),
        vec!["127.0.0.1:1".into()],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let strategy = RoundRobin::new();

    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
  }

  #[test]
  pub fn round_robin_strategy_multiple_addresses() {
    let lb_context = LoadBalancingContext {
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
    let strategy = RoundRobin::new();

    assert_eq!(strategy.resolve_address_index(&lb_context), 1);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 1);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
  }
}
