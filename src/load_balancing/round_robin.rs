use super::{LoadBalancingContext, LoadBalancingStrategy, RequestForwarder};
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
  fn select_backend<'l>(&'l self, _request: &Request<Body>, context: &'l LoadBalancingContext) -> RequestForwarder {
    let mut rrc_handle = self.rrc.lock().unwrap();
    *rrc_handle = (*rrc_handle + 1) % context.backend_addresses.len() as u32;
    let address = &context.backend_addresses[*rrc_handle as usize];
    RequestForwarder::new(address)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn round_robin_strategy_single_address() {
    let request = Request::builder().body(Body::empty()).unwrap();
    let address = "127.0.0.1:1";
    let context = LoadBalancingContext {
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      backend_addresses: &[address.into()],
    };
    let strategy = RoundRobin::new();

    assert_eq!(strategy.select_backend(&request, &context).address, address);
    assert_eq!(strategy.select_backend(&request, &context).address, address);
    assert_eq!(strategy.select_backend(&request, &context).address, address);
  }

  #[test]
  pub fn round_robin_strategy_multiple_addresses() {
    let request = Request::builder().body(Body::empty()).unwrap();
    let address_1 = "127.0.0.1:1";
    let address_2 = "127.0.0.1:2";
    let context = LoadBalancingContext {
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      backend_addresses: &[address_1.into(), address_2.into()],
    };
    let strategy = RoundRobin::new();

    assert_eq!(strategy.select_backend(&request, &context).address, address_2);
    assert_eq!(strategy.select_backend(&request, &context).address, address_1);
    assert_eq!(strategy.select_backend(&request, &context).address, address_2);
    assert_eq!(strategy.select_backend(&request, &context).address, address_1);
  }
}
