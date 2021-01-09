use std::net::SocketAddr;

use hyper::{Body, Request};

use crate::server::BackendPool;

pub mod ip_hash;
pub mod random;
pub mod round_robin;
pub mod sticky_cookie;

pub struct LBContext<'a> {
  pub pool: &'a BackendPool,
  pub client_address: &'a SocketAddr,
  pub client_request: &'a Request<Body>,
}

pub trait LBStrategy: std::fmt::Debug {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize;
}
