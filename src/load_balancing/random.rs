use super::{LoadBalanceTarget, LoadBalancingContext, LoadBalancingStrategy};
use async_trait::async_trait;
use hyper::{Body, Request, Response};
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
  fn resolve_address_index<'l>(
    &'l self,
    _request: &Request<Body>,
    context: &'l LoadBalancingContext,
  ) -> LoadBalanceTarget {
    let mut rng = thread_rng();
    let index = rng.gen_range(0..context.pool.addresses.len());
    LoadBalanceTarget::new(index)
  }
}
