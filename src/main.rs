use clap::{App, Arg};
use listeners::{AcceptorProducer, Https};
use server::{BackendPoolConfig, SharedData};
use std::io;
use std::{path::Path, sync::Arc};
use tokio::try_join;
use tokio_rustls::rustls::{NoClientAuth, ResolvesServerCertUsingSNI, ServerConfig};
use crate::configuration::BackendConfigWatcher;

mod lb_strategy;
mod listeners;
mod logging;
mod middleware;
mod server;
mod tls;
mod configuration;

const LOCAL_HTTP_ADDRESS: &str = "0.0.0.0:80";
const LOCAL_HTTPS_ADDRESS: &str = "0.0.0.0:443";

#[tokio::main]
pub async fn main() -> Result<(), io::Error> {
  let matches = App::new("Another Rust Load Balancer")
      .version("0.1")
      .about("It's basically just another rust load balancer")
      .arg(Arg::with_name("backend")
          .short("b")
          .long("backend")
          .value_name("TOML FILE")
          .help("The path to the backend toml configuration.")
          .required(true)
          .takes_value(true))
      .get_matches();
  let backend_toml = matches.value_of("backend").unwrap().to_string();

  logging::initialize();

  let mut config = BackendConfigWatcher::new(backend_toml);
  config.watch_config_and_apply(start_listening).await
    .unwrap_or(Ok(()))
}

pub async fn start_listening(shared_date: Arc<SharedData>) -> Result<(), io::Error> {
  log::info!("Started listening for {} backends ...", shared_date.backend_pools.len());
  try_join!(
    listen_for_http_request(shared_date.clone()),
    listen_for_https_request(shared_date.clone())
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
    match &pool.config {
      BackendPoolConfig::HttpsConfig {
        certificate_path,
        private_key_path,
      } => tls::add_certificate(
        &mut cert_resolver,
        pool.host.as_str(),
        Path::new(certificate_path.as_str()),
        Path::new(private_key_path.as_str()),
      ),
      _ => Ok(()),
    }?
  }
  tls_config.cert_resolver = Arc::new(cert_resolver);

  let https = Https { tls_config };
  let acceptor = https.produce_acceptor(LOCAL_HTTPS_ADDRESS).await?;

  server::create(acceptor, shared_data, true).await
}
