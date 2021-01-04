use hyper::Client;
use lb_strategies::RandomStrategy;
use listeners::{AcceptorProducer, Https};
use server::{BackendPool, BackendPoolConfig, SharedData};

use std::io::{self, ErrorKind::InvalidData};
use std::vec;
use std::{fs::File, io::BufReader, path::Path, sync::Arc};
use tokio::try_join;
use tokio_rustls::rustls::{
  internal::pemfile::{certs, rsa_private_keys},
  sign::{CertifiedKey, RSASigningKey},
  Certificate, NoClientAuth, PrivateKey, ResolvesServerCertUsingSNI, ServerConfig,
};

mod lb_strategies;
mod listeners;
mod logging;
mod server;

const LOCAL_HTTP_ADDRESS: &str = "0.0.0.0:80";
const LOCAL_HTTPS_ADDRESS: &str = "0.0.0.0:443";

#[tokio::main]
pub async fn main() -> Result<(), io::Error> {
  logging::initialize();

  // let round_robin_counter = Arc::new(Mutex::new(0));
  // let rrc_handle1 = round_robin_counter.clone();
  // let rrc_handle2 = round_robin_counter.clone();

  let backend_pools = vec![
    BackendPool {
      host: "whoami.localhost",
      addresses: vec!["127.0.0.1:8084", "127.0.0.1:8085", "127.0.0.1:8086"],
      config: BackendPoolConfig::HttpConfig {},
      strategy: Box::new(RandomStrategy::new()),
      client: Arc::new(Client::new()),
    },
    BackendPool {
      host: "httpbin.localhost",
      addresses: vec!["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"],
      config: BackendPoolConfig::HttpConfig {},
      strategy: Box::new(RandomStrategy::new()),
      client: Arc::new(Client::new()),
    },
    BackendPool {
      host: "https.localhost",
      addresses: vec!["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"],
      config: BackendPoolConfig::HttpsConfig {
        certificate_path: "x509/https.localhost.cer",
        private_key_path: "x509/https.localhost.key",
      },
      strategy: Box::new(RandomStrategy::new()),
      client: Arc::new(Client::new()),
    },
    BackendPool {
      host: "www.arlb.de",
      addresses: vec!["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"],
      config: BackendPoolConfig::HttpsConfig {
        certificate_path: "x509/www.arlb.de.cer",
        private_key_path: "x509/www.arlb.de.key",
      },
      strategy: Box::new(RandomStrategy::new()),
      client: Arc::new(Client::new()),
    },
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
      } => add_certificate(
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

fn add_certificate(
  cert_resolver: &mut ResolvesServerCertUsingSNI,
  dns_name: &str,
  certificate_path: &Path,
  private_key_path: &Path,
) -> Result<(), io::Error> {
  let certificates = load_certs(certificate_path)?;
  let private_key = load_key(private_key_path)?;
  let private_key = RSASigningKey::new(&private_key).map_err(|_| io::Error::new(InvalidData, "invalid rsa key"))?;
  let certificate_key = CertifiedKey::new(certificates, Arc::new(Box::new(private_key)));
  cert_resolver
    .add(dns_name, certificate_key)
    .map_err(|e| io::Error::new(InvalidData, e))
}

fn load_certs(path: &Path) -> io::Result<Vec<Certificate>> {
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  certs(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid cert"))
}

fn load_key(path: &Path) -> io::Result<PrivateKey> {
  let mut keys = load_keys(path)?;
  Ok(keys.remove(0))
}

fn load_keys(path: &Path) -> io::Result<Vec<PrivateKey>> {
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  rsa_private_keys(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid key"))
}
