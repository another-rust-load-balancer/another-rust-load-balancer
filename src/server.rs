use crate::{
  backend_pool_matcher::BackendPoolMatcher,
  health::Healthiness,
  http_client::StrategyNotifyHttpConnector,
  listeners::RemoteAddress,
  load_balancing::{LoadBalancingContext, LoadBalancingStrategy},
  middleware::RequestHandlerChain,
};
use arc_swap::ArcSwap;
use futures::Future;
use futures::TryFutureExt;
use hyper::{
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
  time::Duration,
  usize,
};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::acme::AcmeHandler;

pub async fn create<'a, I, IE, IO>(
  acceptor: I,
  shared_data: Arc<ArcSwap<SharedData>>,
  https: bool,
) -> Result<(), io::Error>
where
  I: Accept<Conn = IO, Error = IE>,
  IE: Into<Box<dyn std::error::Error + Send + Sync>>,
  IO: AsyncRead + AsyncWrite + Unpin + Send + RemoteAddress + 'static,
{
  let service = make_service_fn(move |stream: &IO| {
    let shared_data = shared_data.load_full();
    let remote_addr = stream.remote_addr().expect("No remote SocketAddr");

    async move {
      Ok::<_, io::Error>(MainService {
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
    host: String,
    certificate_path: String,
    private_key_path: String,
  },
}

pub struct BackendPoolBuilder {
  matcher: BackendPoolMatcher,
  addresses: Vec<(String, Arc<ArcSwap<Healthiness>>)>,
  strategy: Box<dyn LoadBalancingStrategy>,
  config: BackendPoolConfig,
  chain: RequestHandlerChain,
  pool_idle_timeout: Option<Duration>,
  pool_max_idle_per_host: Option<usize>,
}

impl BackendPoolBuilder {
  pub fn new(
    matcher: BackendPoolMatcher,
    addresses: Vec<(String, Arc<ArcSwap<Healthiness>>)>,
    strategy: Box<dyn LoadBalancingStrategy>,
    config: BackendPoolConfig,
    chain: RequestHandlerChain,
  ) -> BackendPoolBuilder {
    BackendPoolBuilder {
      matcher,
      addresses,
      strategy,
      config,
      chain,
      pool_idle_timeout: None,
      pool_max_idle_per_host: None,
    }
  }

  pub fn pool_idle_timeout(&mut self, duration: Duration) -> &BackendPoolBuilder {
    self.pool_idle_timeout = Some(duration);
    self
  }

  pub fn pool_max_idle_per_host(&mut self, max_idle: usize) -> &BackendPoolBuilder {
    self.pool_max_idle_per_host = Some(max_idle);
    self
  }

  pub fn build(self) -> BackendPool {
    let mut client_builder = Client::builder();
    if let Some(pool_idle_timeout) = self.pool_idle_timeout {
      client_builder.pool_idle_timeout(pool_idle_timeout);
    }
    if let Some(pool_max_idle_per_host) = self.pool_max_idle_per_host {
      client_builder.pool_max_idle_per_host(pool_max_idle_per_host);
    }

    let strategy = Arc::new(self.strategy);
    let client: Client<_, Body> = client_builder.build(StrategyNotifyHttpConnector::new(strategy.clone()));

    BackendPool {
      matcher: self.matcher,
      addresses: self.addresses,
      strategy,
      config: self.config,
      chain: self.chain,
      client,
    }
  }
}

#[derive(Debug)]
pub struct BackendPool {
  pub matcher: BackendPoolMatcher,
  pub addresses: Vec<(String, Arc<ArcSwap<Healthiness>>)>,
  pub strategy: Arc<Box<dyn LoadBalancingStrategy>>,
  pub config: BackendPoolConfig,
  pub client: Client<StrategyNotifyHttpConnector, Body>,
  pub chain: RequestHandlerChain,
}

impl PartialEq for BackendPool {
  fn eq(&self, other: &Self) -> bool {
    self.matcher.eq(&other.matcher)
  }
}

pub struct SharedData {
  pub acme_handler: Arc<AcmeHandler>,
  pub backend_pools: Vec<Arc<BackendPool>>,
}

pub struct MainService {
  request_https: bool,
  client_address: SocketAddr,
  shared_data: Arc<SharedData>,
}

pub fn bad_request() -> Response<Body> {
  Response::builder()
    .status(StatusCode::BAD_REQUEST)
    .body(Body::empty())
    .unwrap()
}

pub fn not_found() -> Response<Body> {
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

impl MainService {
  fn pool_by_req(&self, client_request: &Request<Body>) -> Option<Arc<BackendPool>> {
    self
      .shared_data
      .backend_pools
      .iter()
      .find(|pool| pool.matcher.matches(client_request))
      .cloned()
  }

  fn matches_pool_config(&self, config: &BackendPoolConfig) -> bool {
    match config {
      BackendPoolConfig::HttpConfig {} if self.request_https => false,
      BackendPoolConfig::HttpsConfig { .. } if !self.request_https => false,
      _ => true,
    }
  }
}

impl Service<Request<Body>> for MainService {
  type Response = Response<Body>;
  type Error = hyper::Error;

  // let's allow this complex type. A refactor would make it more complicated due to the used trait types
  #[allow(clippy::type_complexity)]
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    if self.shared_data.acme_handler.is_challenge(&request) {
      let acme_handler = self.shared_data.acme_handler.clone();
      return Box::pin(async move {
        let response = acme_handler.respond_to_challenge(request).await;
        Ok(response)
      })
    }

    debug!("{:#?} {} {}", request.version(), request.method(), request.uri());
    match self.pool_by_req(&request) {
      Some(pool) if self.matches_pool_config(&pool.config) => {
        let client_address = self.client_address;

        Box::pin(async move {
          // clone, filter, map, LoadBalancingContext:backend_addresses
          let healthy_addresses: Vec<String> = pool
            .addresses
            .clone()
            .into_iter()
            .filter(|(_, healthiness)| healthiness.load().as_ref() == &Healthiness::Healthy)
            .map(|(address, _)| address)
            .collect();

          // we don't have any healthy addresses, so don't call load balancer strategy and abort early
          // middlewares are also not running
          if healthy_addresses.is_empty() {
            Ok(bad_gateway())
          } else {
            let context = LoadBalancingContext {
              client_address: &client_address,
              backend_addresses: &healthy_addresses,
            };
            let backend = pool.strategy.select_backend(&request, &context);
            let result = backend
              .forward_request(request, &pool.chain, &context, &pool.client)
              .await;
            Ok(result)
          }
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

  fn generate_test_service(host: String, request_https: bool) -> MainService {
    MainService {
      request_https,
      client_address: "127.0.0.1:3000".parse().unwrap(),
      shared_data: Arc::new(SharedData {
        backend_pools: vec![Arc::new(
          BackendPoolBuilder::new(
            BackendPoolMatcher::Host(host),
            vec![(
              "127.0.0.1:8084".into(),
              Arc::new(ArcSwap::from_pointee(Healthiness::Healthy)),
            )],
            Box::new(Random::new()),
            BackendPoolConfig::HttpConfig {},
            RequestHandlerChain::Empty,
          )
          .build(),
        )],
        acme_handler: Arc::new(AcmeHandler::new(String::new(), String::new()))
      }),
    }
  }

  #[test]
  fn pool_by_req_no_matching_pool() {
    let service = generate_test_service("whoami.localhost".into(), false);

    let request = Request::builder()
      .header("host", "whoami.de")
      .body(Body::empty())
      .unwrap();

    let pool = service.pool_by_req(&request);

    assert_eq!(pool.is_none(), true);
  }
  #[test]
  fn pool_by_req_matching_pool() {
    let service = generate_test_service("whoami.localhost".into(), false);
    let request = Request::builder()
      .header("host", "whoami.localhost")
      .body(Body::empty())
      .unwrap();

    let pool = service.pool_by_req(&request);

    assert_eq!(*pool.unwrap(), *service.shared_data.backend_pools[0]);
  }

  #[test]
  fn matches_pool_config() {
    let http_config = BackendPoolConfig::HttpConfig {};
    let https_service = generate_test_service("whoami.localhost".into(), true);
    let http_service = generate_test_service("whoami.localhost".into(), false);
    let https_config = BackendPoolConfig::HttpsConfig {
      host: "whoami.localhost".into(),
      certificate_path: "some/certificate/path".into(),
      private_key_path: "some/private/key/path".into(),
    };

    assert_eq!(http_service.matches_pool_config(&https_config), false);
    assert_eq!(http_service.matches_pool_config(&http_config), true);

    assert_eq!(https_service.matches_pool_config(&https_config), true);
    assert_eq!(https_service.matches_pool_config(&http_config), false);
  }
}
