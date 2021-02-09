use arc_swap::{access::Map, ArcSwap};
use clap::{App, Arg};
use configuration::{read_config, watch_config};
use listeners::{AcceptorProducer, Https};
use server::{Scheme, SharedData};
use std::{io, sync::Arc};
use tls::ReconfigurableCertificateResolver;
use tokio::try_join;
use tokio_rustls::rustls::{NoClientAuth, ServerConfig};

mod acme;
mod backend_pool_matcher;
mod configuration;
mod error_response;
mod health;
mod http_client;
mod listeners;
mod load_balancing;
mod logging;
mod middleware;
mod server;
mod tls;
mod utils;

// Dual Stack if /proc/sys/net/ipv6/bindv6only has default value 0
// rf https://man7.org/linux/man-pages/man7/ipv6.7.html
const LOCAL_HTTP_ADDRESS: &str = "[::]:80";
const LOCAL_HTTPS_ADDRESS: &str = "[::]:443";

#[tokio::main]
pub async fn main() -> Result<(), io::Error> {
  let matches = App::new("Another Rust Load Balancer")
    .version("0.1")
    .about("It's basically just another rust load balancer")
    .arg(
      Arg::with_name("backend")
        .short("b")
        .long("backend")
        .value_name("TOML FILE")
        .help("The path to the backend toml configuration.")
        .required(true)
        .takes_value(true),
    )
    .get_matches();
  let backend = matches.value_of("backend").unwrap().to_string();

  logging::initialize();

  let config = read_config(&backend).await?;
  try_join!(
    watch_config(backend, config.clone()),
    watch_health(config.clone()),
    listen_for_http_request(config.clone()),
    listen_for_https_request(config.clone())
  )?;
  Ok(())
}

async fn watch_health(shared_data: Arc<ArcSwap<SharedData>>) -> Result<(), io::Error> {
  health::watch_health(shared_data).await;
  Ok(())
}

async fn listen_for_http_request(shared_data: Arc<ArcSwap<SharedData>>) -> Result<(), io::Error> {
  let http = listeners::Http;
  let acceptor = http.produce_acceptor(LOCAL_HTTP_ADDRESS).await?;

  server::create(acceptor, shared_data, Scheme::HTTP).await
}

async fn listen_for_https_request(shared_data: Arc<ArcSwap<SharedData>>) -> Result<(), io::Error> {
  let mut tls_config = ServerConfig::new(NoClientAuth::new());
  let certificates = Map::new(shared_data.clone(), |it: &SharedData| &it.certificates);
  let cert_resolver = ReconfigurableCertificateResolver::new(certificates);
  tls_config.cert_resolver = Arc::new(cert_resolver);

  let https = Https { tls_config };
  let acceptor = https.produce_acceptor(LOCAL_HTTPS_ADDRESS).await?;

  server::create(acceptor, shared_data, Scheme::HTTPS).await
}
