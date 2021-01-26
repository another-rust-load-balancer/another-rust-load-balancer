use super::{LoadBalancingContext, LoadBalancingStrategy, RequestForwarder};
use async_trait::async_trait;
use hyper::{Body, Request};
use rand::{thread_rng, Rng};

#[derive(Debug)]
pub struct Random {}

impl Random {
  pub fn new() -> Random {
    Random {}
  }
}

#[async_trait]
impl LoadBalancingStrategy for Random {
  fn select_backend<'l>(&'l self, _request: &Request<Body>, context: &'l LoadBalancingContext) -> RequestForwarder {
    let mut rng = thread_rng();
    let index = rng.gen_range(0..context.backend_addresses.len());
    let address = &context.backend_addresses[index];
    RequestForwarder::new(address)
  }
}
