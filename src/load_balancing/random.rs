use super::{LoadBalancingContext, LoadBalancingStrategy};
use rand::{thread_rng, Rng};

#[derive(Debug)]
pub struct Random {}

impl Random {
  pub fn new() -> Random {
    Random {}
  }
}

impl LoadBalancingStrategy for Random {
  fn resolve_address_index(&self, lb_context: &LoadBalancingContext) -> usize {
    let mut rng = thread_rng();
    rng.gen_range(0..lb_context.pool.addresses.len())
  }
}
