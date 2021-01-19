use crate::{
  listeners::RemoteAddress,
  load_balancing::{self, LoadBalancingContext, LoadBalancingStrategy},
  middleware::RequestHandlerChain,
};
use futures::Future;
use futures::TryFutureExt;
use hyper::{
  client::HttpConnector,
  server::accept::Accept,
  service::{make_service_fn, Service},
  Body, Client, Request, Response, Server, StatusCode,
};
use log::debug;
use std::{
  io,
  net::SocketAddr,
  pin::Pin,
  sync::Arc,
  task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};
use std::sync::RwLock;

pub async fn create<'a, I, IE, IO>(acceptor: I, shared_data: Arc<RwLock<Arc<SharedData>>>, https: bool) -> Result<(), io::Error>
where
  I: Accept<Conn = IO, Error = IE>,
  IE: Into<Box<dyn std::error::Error + Send + Sync>>,
  IO: AsyncRead + AsyncWrite + Unpin + Send + RemoteAddress + 'static,
{
  let service = make_service_fn(move |stream: &IO| {
    let shared_data = (*shared_data.read().unwrap()).clone();
    let remote_addr = stream.remote_addr().expect("No remote SocketAddr");

    async move {
      Ok::<_, io::Error>(LoadBalanceService {
        client_address: remote_addr,
        shared_data,
        request_https: https,
      })
    }
  });
  Server::builder(acceptor)
    .serve(service)
    .map_err(|e| {
      let msg = format!("Failed to listen server: {}", e);
      io::Error::new(io::ErrorKind::Other, msg)
    })
    .await
}

#[derive(Debug, Eq, PartialEq)]
pub enum BackendPoolConfig {
  HttpConfig {},
  HttpsConfig {
    certificate_path: String,
    private_key_path: String,
  },
}

#[derive(Debug)]
pub struct BackendPool {
  pub host: String,
  pub addresses: Vec<String>,
  pub strategy: Box<dyn LoadBalancingStrategy>,
  pub config: BackendPoolConfig,
  pub client: Arc<Client<HttpConnector, Body>>,
  pub chain: Arc<RequestHandlerChain>,
}

impl PartialEq for BackendPool {
  fn eq(&self, other: &Self) -> bool {
    self.host.eq(other.host.as_str())
  }
}

impl BackendPool {
  pub fn new(
    host: String,
    addresses: Vec<String>,
    strategy: Box<dyn LoadBalancingStrategy>,
    config: BackendPoolConfig,
    chain: RequestHandlerChain,
  ) -> BackendPool {
    BackendPool {
      host,
      addresses,
      strategy,
      config,
      client: Arc::new(Client::new()),
      chain: Arc::new(chain),
    }
  }
}

pub struct SharedData {
  pub backend_pools: Vec<Arc<BackendPool>>,
}

pub struct LoadBalanceService {
  request_https: bool,
  client_address: SocketAddr,
  shared_data: Arc<SharedData>,
}

fn not_found() -> Response<Body> {
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(Body::from("404 - page not found"))
    .unwrap()
}

pub fn bad_gateway() -> Response<Body> {
  Response::builder()
    .status(StatusCode::BAD_GATEWAY)
    .body(Body::empty())
    .unwrap()
}

impl LoadBalanceService {
  fn pool_by_req<T>(&self, client_request: &Request<T>) -> Option<Arc<BackendPool>> {
    let host_header = client_request.headers().get("host")?;

    self
      .shared_data
      .backend_pools
      .iter()
      .find(|pool| pool.host.as_str() == host_header)
      .map(|pool| pool.clone())
  }

  fn matches_pool_config(&self, config: &BackendPoolConfig) -> bool {
    match config {
      BackendPoolConfig::HttpConfig {} if self.request_https => false,
      BackendPoolConfig::HttpsConfig { .. } if !self.request_https => false,
      _ => true,
    }
  }
}

impl Service<Request<Body>> for LoadBalanceService {
  type Response = Response<Body>;
  type Error = hyper::Error;

  // let's allow this complex type. A refactor would make it more complicated due to the used trait types
  #[allow(clippy::type_complexity)]
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    debug!("{:#?} {} {}", request.version(), request.method(), request.uri());
    match self.pool_by_req(&request) {
      Some(pool) if self.matches_pool_config(&pool.config) => {
        let client_address = self.client_address;
        Box::pin(async move {
          let context = LoadBalancingContext { client_address, pool };
          let result =
            load_balancing::handle_request(request, &context.pool.strategy, &context.pool.chain, &context).await;
          Ok(result)
        })
      }
      _ => Box::pin(async { Ok(not_found()) }),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::load_balancing::random::Random;

  fn generate_test_service(host: String, request_https: bool) -> LoadBalanceService {
    LoadBalanceService {
      request_https,
      client_address: "127.0.0.1:3000".parse().unwrap(),
      shared_data: Arc::new(SharedData {
        backend_pools: vec![Arc::new(BackendPool::new(
          host,
          vec!["127.0.0.1:8084".into()],
          Box::new(Random::new()),
          BackendPoolConfig::HttpConfig {},
          RequestHandlerChain::Empty,
        ))],
      }),
    }
  }

  #[test]
  fn pool_by_req_no_matching_pool() {
    let service = generate_test_service("whoami.localhost".into(), false);

    let request = Request::builder().header("host", "whoami.de").body(()).unwrap();

    let pool = service.pool_by_req(&request);

    assert_eq!(pool.is_none(), true);
  }
  #[test]
  fn pool_by_req_matching_pool() {
    let service = generate_test_service("whoami.localhost".into(), false);
    let request = Request::builder().header("host", "whoami.localhost").body(()).unwrap();

    let pool = service.pool_by_req(&request);

    assert_eq!(*pool.unwrap(), *service.shared_data.backend_pools[0]);
  }

  #[test]
  fn matches_pool_config() {
    let http_config = BackendPoolConfig::HttpConfig {};
    let https_service = generate_test_service("whoami.localhost".into(), true);
    let http_service = generate_test_service("whoami.localhost".into(), false);
    let https_config = BackendPoolConfig::HttpsConfig {
      certificate_path: "some/certificate/path".into(),
      private_key_path: "some/private/key/path".into(),
    };

    assert_eq!(http_service.matches_pool_config(&https_config), false);
    assert_eq!(http_service.matches_pool_config(&http_config), true);

    assert_eq!(https_service.matches_pool_config(&https_config), true);
    assert_eq!(https_service.matches_pool_config(&http_config), false);
  }
}
