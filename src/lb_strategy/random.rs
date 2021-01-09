use rand::{thread_rng, Rng};

use super::{LBContext, LBStrategy};

#[derive(Debug)]
pub struct Random {}

impl Random {
  pub fn new() -> Random {
    Random {}
  }
}

impl LBStrategy for Random {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    let mut rng = thread_rng();
    rng.gen_range(0..lb_context.pool.addresses.len())
  }
}
