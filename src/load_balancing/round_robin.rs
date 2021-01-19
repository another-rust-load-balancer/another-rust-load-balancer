use super::{LoadBalanceTarget, LoadBalancingContext, LoadBalancingStrategy};
use hyper::{Body, Request};
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
  fn resolve_address_index<'l>(
    &'l self,
    _request: &Request<Body>,
    lb_context: &'l LoadBalancingContext,
  ) -> LoadBalanceTarget {
    let mut rrc_handle = self.rrc.lock().unwrap();
    *rrc_handle = (*rrc_handle + 1) % lb_context.pool.addresses.len() as u32;
    LoadBalanceTarget::new(*rrc_handle as usize)
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    middleware::RequestHandlerChain,
    server::{BackendPool, BackendPoolConfig},
  };

  use super::*;

  #[test]
  pub fn round_robin_strategy_single_address() {
    let request = Request::builder().body(Body::empty()).unwrap();
    let context = LoadBalancingContext {
      client_address: "127.0.0.1:3000".parse().unwrap(),
      pool: Arc::new(BackendPool::new(
        "whoami.localhost".into(),
        vec!["127.0.0.1:1".into()],
        Box::new(RoundRobin::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      )),
    };
    let strategy = RoundRobin::new();

    assert_eq!(strategy.resolve_address_index(&request, &context).index, 0);
    assert_eq!(strategy.resolve_address_index(&request, &context).index, 0);
    assert_eq!(strategy.resolve_address_index(&request, &context).index, 0);
  }

  #[test]
  pub fn round_robin_strategy_multiple_addresses() {
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
    let strategy = RoundRobin::new();

    assert_eq!(strategy.resolve_address_index(&request, &context).index, 1);
    assert_eq!(strategy.resolve_address_index(&request, &context).index, 0);
    assert_eq!(strategy.resolve_address_index(&request, &context).index, 1);
    assert_eq!(strategy.resolve_address_index(&request, &context).index, 0);
  }
}
