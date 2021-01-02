use rand::{thread_rng, Rng};

// TODO stream: &AddrStream is missing from the arguments
pub trait LBStrategy {
  fn resolve_address_index(&self, address_count: usize) -> usize;
}
// TODO Add more strategies (IP-Hash, Round-Robbin)
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
  fn resolve_address_index(&self, address_count: usize) -> usize {
    let mut rng = thread_rng();
    rng.gen_range(0..address_count)
  }
}
