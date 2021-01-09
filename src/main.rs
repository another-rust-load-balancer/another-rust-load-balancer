use lb_strategies::{IPHashStrategy, RandomStrategy, RoundRobinStrategy};
use listeners::{AcceptorProducer, Https};
use middleware::compression::Compression;
use middleware::RequestHandlerChain;
use server::{BackendPool, BackendPoolConfig, SharedData};
use std::io;
use std::vec;
use std::{path::Path, sync::Arc};
use tokio::try_join;
use tokio_rustls::rustls::{NoClientAuth, ResolvesServerCertUsingSNI, ServerConfig};

mod lb_strategies;
mod listeners;
mod logging;
mod middleware;
mod server;
mod tls;

const LOCAL_HTTP_ADDRESS: &str = "0.0.0.0:80";
const LOCAL_HTTPS_ADDRESS: &str = "0.0.0.0:443";

#[tokio::main]
pub async fn main() -> Result<(), io::Error> {
  logging::initialize();

  // let round_robin_counter = Arc::new(Mutex::new(0));
  // let rrc_handle1 = round_robin_counter.clone();
  // let rrc_handle2 = round_robin_counter.clone();

  let backend_pools = vec![
    BackendPool::new(
      "whoami.localhost",
      vec!["127.0.0.1:8084", "127.0.0.1:8085", "127.0.0.1:8086"],
      Box::new(RoundRobinStrategy::new()),
      BackendPoolConfig::HttpConfig {},
      RequestHandlerChain::Entry {
        handler: Box::new(Compression {}),
        next: Box::new(RequestHandlerChain::Empty),
      },
    ),
    BackendPool::new(
      "httpbin.localhost",
      vec!["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"],
      Box::new(RandomStrategy::new()),
      BackendPoolConfig::HttpConfig {},
      RequestHandlerChain::Entry {
        handler: Box::new(Compression {}),
        next: Box::new(RequestHandlerChain::Empty),
      },
    ),
    BackendPool::new(
      "https.localhost",
      vec!["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"],
      Box::new(IPHashStrategy::new()),
      BackendPoolConfig::HttpsConfig {
        certificate_path: "x509/https.localhost.cer",
        private_key_path: "x509/https.localhost.key",
      },
      RequestHandlerChain::Entry {
        handler: Box::new(Compression {}),
        next: Box::new(RequestHandlerChain::Empty),
      },
    ),
    BackendPool::new(
      "www.arlb.de",
      vec!["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"],
      Box::new(RandomStrategy::new()),
      BackendPoolConfig::HttpsConfig {
        certificate_path: "x509/www.arlb.de.cer",
        private_key_path: "x509/www.arlb.de.key",
      },
      RequestHandlerChain::Entry {
        handler: Box::new(Compression {}),
        next: Box::new(RequestHandlerChain::Empty),
      },
    ),
  ];
  let shared_data = Arc::new(SharedData { backend_pools });

  try_join!(
    listen_for_http_request(shared_data.clone()),
    listen_for_https_request(shared_data.clone())
  )?;

  Ok(())
}

async fn listen_for_http_request(shared_data: Arc<SharedData>) -> Result<(), io::Error> {
  let http = listeners::Http {};
  let acceptor = http.produce_acceptor(LOCAL_HTTP_ADDRESS).await?;

  server::create(acceptor, shared_data, false).await
}

async fn listen_for_https_request(shared_data: Arc<SharedData>) -> Result<(), io::Error> {
  let mut tls_config = ServerConfig::new(NoClientAuth::new());
  let mut cert_resolver = ResolvesServerCertUsingSNI::new();

  for pool in &shared_data.backend_pools {
    match pool.config {
      BackendPoolConfig::HttpsConfig {
        certificate_path,
        private_key_path,
      } => tls::add_certificate(
        &mut cert_resolver,
        pool.host,
        Path::new(certificate_path),
        Path::new(private_key_path),
      ),
      _ => Ok(()),
    }?
  }
  tls_config.cert_resolver = Arc::new(cert_resolver);

  let https = Https { tls_config };
  let acceptor = https.produce_acceptor(LOCAL_HTTPS_ADDRESS).await?;

  server::create(acceptor, shared_data, true).await
}
