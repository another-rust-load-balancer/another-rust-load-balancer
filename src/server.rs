use std::{
  io,
  net::SocketAddr,
  pin::Pin,
  str,
  sync::Arc,
  task::{Context, Poll},
};

use futures::future::*;
use hyper::{
  server::{accept::Accept, conn::AddrStream},
  service::{make_service_fn, Service},
  Body, Client, Request, Response, Server, StatusCode, Uri,
};
use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::lb_strategies::LBStrategy;

pub async fn create<'a, I, IE, IO>(acceptor: I, shared_data: Arc<SharedData>, https: bool) -> Result<(), io::Error>
where
  I: Accept<Conn = IO, Error = IE>,
  IE: Into<Box<dyn std::error::Error + Send + Sync>>,
  IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
  let service = make_service_fn(move |_| {
    // let remote_addr = Arc::new(stream.remote_addr());
    // let remote_addr = remote_addr.clone();

    let shared_data = shared_data.clone();

    async move {
      Ok::<_, io::Error>(LoadBalanceService {
        // remote_addr,
        shared_data,
        https,
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

pub enum BackendPoolConfig {
  HttpConfig {},
  HttpsConfig {
    certificate_path: &'static str,
    private_key_path: &'static str,
  },
}

pub struct BackendPool {
  pub host: &'static str,
  pub addresses: Vec<&'static str>,
  pub strategy: Arc<dyn LBStrategy + Send + Sync>,
  pub config: BackendPoolConfig,
}

impl BackendPool {
  pub fn get_address(&self) -> &str {
    let index = self.strategy.resolve_address_index(self.addresses.len());
    return self.addresses[index];
  }
}

pub struct SharedData {
  pub backend_pools: Vec<BackendPool>,
}

pub struct LoadBalanceService {
  https: bool,
  // remote_addr: Arc<SocketAddr>,
  shared_data: Arc<SharedData>,
}

impl Service<Request<Body>> for LoadBalanceService {
  type Response = Response<Body>;
  type Error = hyper::Error;
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

    let host_header = client_request.headers().get("host").unwrap();

    let pool = self
      .shared_data
      .backend_pools
      .iter()
      .find(|pool| pool.host == host_header);

    match pool {
      None => {
        let response = Response::builder()
          .status(StatusCode::NOT_FOUND)
          .body(Body::from("404 - page not found"))
          .unwrap();
        Box::pin(async { Ok(response) })
      }
      Some(pool) => {
        match pool.config {
          BackendPoolConfig::HttpConfig {} if self.https => {
            debug!("HTTP Pool found - but requested via https");

            let response = Response::builder()
              .status(StatusCode::NOT_FOUND)
              .body(Body::from("404 - page not found"))
              .unwrap();
            return Box::pin(async { Ok(response) });
          }
          BackendPoolConfig::HttpsConfig { .. } if !self.https => {
            debug!("HTTPS Pool found - but requested via http");

            let response = Response::builder()
              .status(StatusCode::NOT_FOUND)
              .body(Body::from("404 - page not found"))
              .unwrap();
            return Box::pin(async { Ok(response) });
          }
          _ => (),
        }

        let path = client_request.uri().path_and_query().unwrap().clone();

        let uri = Uri::builder()
          .path_and_query(path)
          .scheme("http")
          .authority(pool.get_address())
          .build()
          .unwrap();

        let backend_req_builder = Request::builder().uri(uri);

        let backend_request = client_request
          .headers()
          .iter()
          .fold(backend_req_builder, |backend_req_builder, (key, val)| {
            backend_req_builder.header(key, val)
          })
          .body(client_request.into_body())
          .unwrap();

        let fut = async {
          let resp = Client::new().request(backend_request).await?;
          Ok(resp)
        };
        Box::pin(fut)
      }
    }
  }
}
