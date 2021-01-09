use cookie::{Cookie, SameSite};
use hyper::{header::COOKIE, Body, Request};
use rand::{thread_rng, Rng};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::{collections::hash_map::DefaultHasher, str};

use crate::{middleware::sticky_cookie_companion::StickyCookieCompanion, server::BackendPool};

pub struct LBContext<'a> {
  pub pool: &'a BackendPool,
  pub client_address: &'a SocketAddr,
  pub client_request: &'a Request<Body>,
}

pub trait LBStrategy: std::fmt::Debug {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize;
}

#[derive(Debug)]
pub struct RandomStrategy {}

impl RandomStrategy {
  pub fn new() -> RandomStrategy {
    RandomStrategy {}
  }
}

impl LBStrategy for RandomStrategy {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    let mut rng = thread_rng();
    rng.gen_range(0..lb_context.pool.addresses.len())
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
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    // finish() does not reset state, so we'll need a new hasher for each request
    let mut hasher = DefaultHasher::new();
    lb_context.client_address.ip().hash(&mut hasher);
    (hasher.finish() % (lb_context.pool.addresses.len() as u64)) as usize
  }
}

#[derive(Debug)]
pub struct RoundRobinStrategy {
  rrc: Arc<Mutex<u32>>,
}

impl RoundRobinStrategy {
  pub fn new() -> RoundRobinStrategy {
    RoundRobinStrategy {
      rrc: Arc::new(Mutex::new(0)),
    }
  }
}

impl LBStrategy for RoundRobinStrategy {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    let mut rrchandle = self.rrc.lock().unwrap();
    *rrchandle = (*rrchandle + 1) % lb_context.pool.addresses.len() as u32;
    *rrchandle as usize
  }
}

#[derive(Debug)]
pub struct StickyCookieStrategyConfig {
  pub cookie_name: &'static str,
  pub secure: bool,
  pub http_only: bool,
  pub same_site: SameSite,
}

// TODO: Implement builder?
#[derive(Debug)]
pub struct StickyCookieStrategy {
  pub config: Arc<StickyCookieStrategyConfig>,
  pub inner: Box<dyn LBStrategy + Send + Sync>,
}

impl StickyCookieStrategy {
  pub fn new(
    cookie_name: &'static str,
    inner: Box<dyn LBStrategy + Send + Sync>,
  ) -> (StickyCookieStrategy, StickyCookieCompanion) {
    let config = Arc::new(StickyCookieStrategyConfig {
      cookie_name,
      http_only: false,
      secure: false,
      same_site: SameSite::None,
    });

    let strategy = StickyCookieStrategy {
      config: config.clone(),
      inner,
    };
    let companion = StickyCookieCompanion { config: config.clone() };

    (strategy, companion)
  }

  fn try_parse_sticky_cookie<'a>(&self, request: &'a Request<Body>) -> Option<Cookie<'a>> {
    let cookie_header = request.headers().get(COOKIE)?;

    cookie_header.to_str().ok()?.split(";").find_map(|cookie_str| {
      let cookie = Cookie::parse(cookie_str).ok()?;
      if cookie.name() == self.config.cookie_name {
        Some(cookie)
      } else {
        None
      }
    })
  }
}

impl LBStrategy for StickyCookieStrategy {
  fn resolve_address_index(&self, lb_context: &LBContext) -> usize {
    self
      .try_parse_sticky_cookie(lb_context.client_request)
      .and_then(|cookie| lb_context.pool.addresses.iter().position(|a| *a == cookie.value()))
      .unwrap_or_else(|| self.inner.resolve_address_index(lb_context))
  }
}

#[cfg(test)]
mod tests {
  use crate::{middleware::RequestHandlerChain, server::BackendPoolConfig};

  use super::*;

  #[test]
  pub fn round_robin_strategy_single_address() {
    let lb_context = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1"],
        Box::new(RoundRobinStrategy::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let strategy = RoundRobinStrategy::new();

    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
  }

  #[test]
  pub fn round_robin_strategy_multiple_addresses() {
    let lb_context = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2"],
        Box::new(RoundRobinStrategy::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let strategy = RoundRobinStrategy::new();

    assert_eq!(strategy.resolve_address_index(&lb_context), 1);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
    assert_eq!(strategy.resolve_address_index(&lb_context), 1);
    assert_eq!(strategy.resolve_address_index(&lb_context), 0);
  }

  #[test]
  pub fn ip_hash_strategy_same_ip() {
    let lb_context = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2"],
        Box::new(RoundRobinStrategy::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let strategy = IPHashStrategy::new();

    let index = strategy.resolve_address_index(&lb_context);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
    assert_eq!(strategy.resolve_address_index(&lb_context), index);
  }

  #[test]
  pub fn ip_hash_strategy_different_ip() {
    let lb_context_1 = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2", "127.0.0.1:3", "127.0.0.1:4"],
        Box::new(RoundRobinStrategy::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };
    let lb_context_2 = LBContext {
      client_request: &Request::builder().body(Body::empty()).unwrap(),
      client_address: &"192.168.0.4:3000".parse().unwrap(),
      pool: &BackendPool::new(
        "whoami.localhost",
        vec!["127.0.0.1:1", "127.0.0.1:2", "127.0.0.1:3", "127.0.0.1:4"],
        Box::new(RoundRobinStrategy::new()),
        BackendPoolConfig::HttpConfig {},
        RequestHandlerChain::Empty,
      ),
    };

    let strategy = IPHashStrategy::new();

    assert_ne!(
      strategy.resolve_address_index(&lb_context_1),
      strategy.resolve_address_index(&lb_context_2)
    );
  }
}
