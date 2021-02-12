use arc_swap::{access::Map, ArcSwap};
use clap::{App, Arg};
use configuration::{read_initial_config, watch_config, RuntimeConfig};
use listeners::{AcceptorProducer, Https};
use server::Scheme;
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

#[tokio::main]
pub async fn main() -> Result<(), io::Error> {
  let matches = App::new("Another Rust Load Balancer")
    .version("1.0")
    .about("It's basically just another rust load balancer")
    .arg(
      Arg::with_name("config")
        .short("c")
        .long("config")
        .value_name("TOML FILE")
        .help("The path to the configuration in TOML format.")
        .required(true)
        .takes_value(true),
    )
    .get_matches();
  let config_path = matches.value_of("config").unwrap().to_string();

  logging::initialize();

  let config = read_initial_config(&config_path).await?;
  try_join!(
    watch_config(config_path, config.clone()),
    watch_health(config.clone()),
    listen_for_http_request(config.clone()),
    listen_for_https_request(config.clone())
  )?;
  Ok(())
}

async fn watch_health(config: Arc<ArcSwap<RuntimeConfig>>) -> Result<(), io::Error> {
  let backend_pools = Map::new(config.clone(), |it: &RuntimeConfig| &it.shared_data.backend_pools);
  let health_interval = Map::new(config, |it: &RuntimeConfig| &it.health_interval);
  health::watch_health(backend_pools, health_interval).await;
  Ok(())
}

async fn listen_for_http_request(config: Arc<ArcSwap<RuntimeConfig>>) -> Result<(), io::Error> {
  let http = listeners::Http;
  let address = config.load().http_address;
  let acceptor = http.produce_acceptor(address).await?;

  server::create(acceptor, config, Scheme::HTTP).await
}

async fn listen_for_https_request(config: Arc<ArcSwap<RuntimeConfig>>) -> Result<(), io::Error> {
  let mut tls_config = ServerConfig::new(NoClientAuth::new());
  let certificates = Map::new(config.clone(), |it: &RuntimeConfig| &it.certificates);
  let cert_resolver = ReconfigurableCertificateResolver::new(certificates);
  tls_config.cert_resolver = Arc::new(cert_resolver);

  let https = Https { tls_config };
  let address = config.load().https_address;
  let acceptor = https.produce_acceptor(address).await?;

  server::create(acceptor, config, Scheme::HTTPS).await
}
