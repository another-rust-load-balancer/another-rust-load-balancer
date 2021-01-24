use std::{collections::HashMap, sync::RwLock};

use hyper::{Body, Request, Uri};
use rand::{thread_rng, Rng};

use super::{LoadBalancingContext, LoadBalancingStrategy, RequestForwarder};

#[derive(Debug)]
pub struct LeastConnection {
  connections: RwLock<HashMap<String, usize>>,
}

impl LeastConnection {
  pub fn new() -> LeastConnection {
    LeastConnection {
      connections: RwLock::new(HashMap::new()),
    }
  }
}

impl LoadBalancingStrategy for LeastConnection {
  fn on_tcp_open(&self, remote: &Uri) {
    if let Some(authority) = remote.authority() {
      let mut connections = self.connections.write().unwrap();
      *connections.entry(authority.to_string()).or_insert(0) += 1;
    }
  }

  fn on_tcp_close(&self, remote: &Uri) {
    if let Some(authority) = remote.authority() {
      let mut connections = self.connections.write().unwrap();
      *connections.entry(authority.to_string()).or_insert(1) -= 1;
    }
  }

  fn select_backend<'l>(&'l self, _request: &Request<Body>, context: &'l LoadBalancingContext) -> RequestForwarder {
    // ok to unwrap - only panics when we panic somewhere else :)
    let connections = self.connections.read().unwrap();

    let address_indices: Vec<usize> = if connections.len() == 0 || context.backend_addresses.len() > connections.len() {
      // if no TCP connections have been opened yet, or some backend servers are not used yet, we'll use them for the next request
      context
        .backend_addresses
        .iter()
        .enumerate()
        .filter(|(_, address)| !connections.contains_key(*address))
        .map(|(index, _)| index)
        .collect()
    } else {
      let backend_address_map: HashMap<_, _> = context
        .backend_addresses
        .iter()
        .enumerate()
        .map(|(index, address)| (address, index))
        .collect();
      let mut least_connections = connections.iter().collect::<Vec<_>>();

      least_connections.sort_by(|a, b| a.1.cmp(b.1));

      let min_connection_count = least_connections[0].1;
      least_connections
        .iter()
        .take_while(|(_, connection_count)| *connection_count == min_connection_count)
        .map(|tuple| tuple.0)
        .map(|address| *backend_address_map.get(address).unwrap())
        .collect()
    };

    if address_indices.len() == 1 {
      RequestForwarder::new(&context.backend_addresses[address_indices[0]])
    } else {
      let mut rng = thread_rng();
      let index = rng.gen_range(0..address_indices.len());
      RequestForwarder::new(&context.backend_addresses[address_indices[index]])
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn least_connection_single_least_address() {
    let request = Request::builder().body(Body::empty()).unwrap();

    let context = LoadBalancingContext {
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      backend_addresses: &mut ["127.0.0.1:1".into(), "127.0.0.1:2".into()],
    };

    let strategy = LeastConnection::new();

    strategy.on_tcp_open(&"127.0.0.1:1".parse().unwrap());

    assert_eq!(
      strategy.select_backend(&request, &context).address,
      &context.backend_addresses[1]
    );
  }

  #[test]
  pub fn least_connection_multiple_least_addresses() {
    let request = Request::builder().body(Body::empty()).unwrap();

    let context = LoadBalancingContext {
      client_address: &"127.0.0.1:3000".parse().unwrap(),
      backend_addresses: &mut ["127.0.0.1:1".into(), "127.0.0.1:2".into(), "127.0.0.1:3".into()],
    };

    let strategy = LeastConnection::new();
    strategy.on_tcp_open(&"127.0.0.1:1".parse().unwrap());

    assert_ne!(
      strategy.select_backend(&request, &context).address,
      &context.backend_addresses[0]
    );
    assert_ne!(
      strategy.select_backend(&request, &context).address,
      &context.backend_addresses[0]
    );
    assert_ne!(
      strategy.select_backend(&request, &context).address,
      &context.backend_addresses[0]
    );
  }
}
