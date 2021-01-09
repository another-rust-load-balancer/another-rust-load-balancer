use std::sync::{Arc, Mutex};

use super::{LBContext, LBStrategy};

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

impl LBStrategy for RoundRobin {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
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
    let lb_context = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1"],
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
    let strategy = RoundRobin::new();

    assert_eq!(strategy.resolve_address_index(&lb_context), 1);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 1);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
  }
}
