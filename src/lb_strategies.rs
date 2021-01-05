use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use rand::{thread_rng, Rng};

pub trait LBStrategy: std::fmt::Debug {
  fn resolve_address_index(&self, address_count: usize, remote_addr: &SocketAddr) -> usize;
}

#[derive(Debug)]
pub struct RandomStrategy {}

impl RandomStrategy {
  pub fn new() -> RandomStrategy {
    RandomStrategy {}
  }
}

impl LBStrategy for RandomStrategy {
  fn resolve_address_index(&self, address_count: usize, _remote_addr: &SocketAddr) -> usize {
    let mut rng = thread_rng();
    rng.gen_range(0..address_count)
  }
}

#[derive(Debug)]
pub struct IPHashStrategy {}

impl IPHashStrategy {
  pub fn new() -> IPHashStrategy {
    IPHashStrategy {}
  }
}

impl LBStrategy for IPHashStrategy {
  fn resolve_address_index(&self, address_count: usize, _remote_addr: &SocketAddr) -> usize {
    let mut hasher = DefaultHasher::new();
    _remote_addr.port().hash(&mut hasher);
    (hasher.finish() % address_count as u64) as usize
  }
}

#[derive(Debug)]
pub struct RoundRobinStrategy {
  rrc: Arc<Mutex<u32>>,
}

impl RoundRobinStrategy {
  pub fn new() -> RoundRobinStrategy {
    RoundRobinStrategy {
      rrc : Arc::new(Mutex::new(0))
    }
  }
}

impl LBStrategy for RoundRobinStrategy {
  fn resolve_address_index(&self, address_count: usize, _remote_addr: &SocketAddr) -> usize {
    let mut rrchandle = self.rrc.lock().unwrap();
    *rrchandle = (*rrchandle +1) % address_count as u32;
    *rrchandle as usize
  }
}