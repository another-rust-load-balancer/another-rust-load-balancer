use crate::{
  lb_strategy::{LBContext, LBStrategy},
  listeners::RemoteAddress,
  middleware::{RequestHandlerChain, RequestHandlerContext},
};
use futures::Future;
use futures::TryFutureExt;
use hyper::{
  client::HttpConnector,
  server::accept::Accept,
  service::{make_service_fn, Service},
  Body, Client, Request, Response, Server, StatusCode, Uri,
};
use log::debug;
use std::{
  io,
  net::SocketAddr,
  pin::Pin,
  str,
  sync::Arc,
  task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};

pub async fn create<'a, I, IE, IO>(acceptor: I, shared_data: Arc<SharedData>, https: bool) -> Result<(), io::Error>
where
  I: Accept<Conn = IO, Error = IE>,
  IE: Into<Box<dyn std::error::Error + Send + Sync>>,
  IO: AsyncRead + AsyncWrite + Unpin + Send + RemoteAddress + 'static,
{
  let service = make_service_fn(move |stream: &IO| {
    let shared_data = shared_data.clone();
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
  pub strategy: Box<dyn LBStrategy + Send + Sync>,
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
    strategy: Box<dyn LBStrategy + Send + Sync>,
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

  pub fn get_address(&self, client_address: &SocketAddr, client_request: &Request<Body>) -> &str {
    let index = self.strategy.resolve_address_index(&LBContext {
      client_request,
      client_address,
      pool: &self,
    });
    self.addresses[index].as_str()
  }
}

pub struct SharedData {
  pub backend_pools: Vec<BackendPool>,
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
  fn pool_by_req<T>(&self, client_request: &Request<T>) -> Option<&BackendPool> {
    let host_header = client_request.headers().get("host")?;

    self
      .shared_data
      .backend_pools
      .iter()
      .find(|pool| pool.host.as_str() == host_header)
  }

  fn matches_pool_config(&self, config: &BackendPoolConfig) -> bool {
    match config {
      BackendPoolConfig::HttpConfig {} if self.request_https => false,
      BackendPoolConfig::HttpsConfig { .. } if !self.request_https => false,
      _ => true,
    }
  }

  fn backend_uri(&self, pool: &BackendPool, client_request: &Request<Body>) -> Uri {
    let path = client_request.uri().path_and_query().unwrap().clone();

    Uri::builder()
      .path_and_query(path)
      .scheme("http")
      .authority(pool.get_address(&self.client_address, &client_request))
      .build()
      .unwrap()
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

  fn call(&mut self, client_request: Request<Body>) -> Self::Future {
    debug!(
      "{:#?} {} {}",
      client_request.version(),
      client_request.method(),
      client_request.uri()
    );
    match self.pool_by_req(&client_request) {
      Some(pool) if self.matches_pool_config(&pool.config) => {
        let context = RequestHandlerContext {
          client_address: self.client_address,
          backend_uri: self.backend_uri(pool, &client_request),
          client: pool.client.clone(),
        };
        let chain = pool.chain.clone();
        Box::pin(async move {
          match chain.handle_request(client_request, &context).await {
            Ok(response) => Ok(response),
            Err(response) => Ok(response),
          }
        })
      }
      _ => Box::pin(async { Ok(not_found()) }),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::lb_strategy::random::Random;

  use super::*;
  use hyper::http::uri::{Authority, Scheme};

  fn generate_test_service(host: String, request_https: bool) -> LoadBalanceService {
    LoadBalanceService {
      request_https,
      client_address: "127.0.0.1:3000".parse().unwrap(),
      shared_data: Arc::new(SharedData {
        backend_pools: vec![BackendPool::new(
          host,
          vec!["127.0.0.1:8084".into()],
          Box::new(Random::new()),
          BackendPoolConfig::HttpConfig {},
          RequestHandlerChain::Empty,
        )],
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

    assert_eq!(*pool.unwrap(), service.shared_data.backend_pools[0]);
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

  #[test]
  fn backend_uri() {
    let service = generate_test_service("whoami.localhost".into(), false);

    let pool = &service.shared_data.backend_pools[0];
    let request = Request::builder()
      .uri("https://www.rust-lang.org/path?param=yolo")
      .header("host", "whoami.localhost")
      .body(Body::empty())
      .unwrap();

    let backend_uri = service.backend_uri(pool, &request);

    assert_eq!(backend_uri.authority(), Some(&Authority::from_static("127.0.0.1:8084")));
    assert_eq!(backend_uri.path(), "/path");
    assert_eq!(backend_uri.query(), Some("param=yolo"));
    assert_eq!(backend_uri.scheme(), Some(&Scheme::HTTP));
  }
}
