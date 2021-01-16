use crate::server::BackendPool;
use hyper::{Body, Request};
use std::net::SocketAddr;

pub mod ip_hash;
pub mod random;
pub mod round_robin;
pub mod sticky_cookie;

pub struct LoadBalancingContext<'a> {
  pub pool: &'a BackendPool,
  pub client_address: &'a SocketAddr,
  pub client_request: &'a Request<Body>,
}

pub trait LoadBalancingStrategy: std::fmt::Debug {
  fn resolve_address_index(&self, context: &LoadBalancingContext) -> usize;
}
