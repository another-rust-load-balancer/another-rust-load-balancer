use std::net::SocketAddr;

use rand::{thread_rng, Rng};

pub trait LBStrategy: std::fmt::Debug {
  fn resolve_address_index(&self, address_count: usize, remote_addr: &SocketAddr) -> usize;
}

#[derive(Debug)]
pub struct RandomStrategy {
  // TODO save rng for all calls -> rng: ThreadRng,
}

impl RandomStrategy {
  pub fn new() -> RandomStrategy {
    RandomStrategy {
      // rng: rand::thread_rng(),
    }
  }
}

impl LBStrategy for RandomStrategy {
  fn resolve_address_index(&self, address_count: usize, _remote_addr: &SocketAddr) -> usize {
    let mut rng = thread_rng();
    rng.gen_range(0..address_count)
  }
}
