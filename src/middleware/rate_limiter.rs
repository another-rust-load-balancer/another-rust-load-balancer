use super::{Context, Middleware};
use async_trait::async_trait;
use hyper::{Body, Request, Response, StatusCode};
use linked_hash_map::LinkedHashMap;
use log::warn;
use std::{net::SocketAddr, sync::Mutex, time::SystemTime};

#[derive(Debug)]
pub struct RateLimiter {
  connections: Mutex<LinkedHashMap<SocketAddr, (i64, SystemTime)>>,
  pub limit: i64,
}

impl RateLimiter {
  pub fn new(limit: i64) -> RateLimiter {
    RateLimiter {
      connections: Mutex::new(LinkedHashMap::new()),
      limit: limit,
    }
  }
}

#[async_trait]
impl Middleware for RateLimiter {
  async fn modify_request(
    &self,
    request: Request<Body>,
    context: &Context<'_>,
  ) -> Result<Request<Body>, Response<Body>> {
    let mut connections = self.connections.lock().unwrap(); //blocks thread - problem?
    let coll = connections.clone();
    let client_address = context.client_address;

    let a = coll.get(client_address);
    let time_now = SystemTime::now();
    if let Some((counter, time)) = a {
      let time_passed = time_now.duration_since(*time).unwrap(); // todo remove unwrap
      if time_passed.as_secs() < 1 {
        //todo make 1 configurable
        let counter = *counter + 1;
        if counter <= self.limit {
          connections.insert(*client_address, (counter, time_now));
          Ok(request)
        } else {
          let response = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::empty())
            .unwrap();
          Err(response)
        }
      } else {
        connections.insert(*client_address, (1, time_now));
        Ok(request)
      }
    } else {
      connections.insert(*client_address, (1, time_now));
      Ok(request)
    }
    // drop(connections);
  }
}

/*
Limit number of requests (to a host) allowed per IP per time step (sec/min)

How:

*/
