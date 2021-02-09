use crate::{
  acme::AcmeHandler,
  health::{HealthConfig, Healthiness},
  load_balancing::{
    ip_hash::IPHash, least_connection::LeastConnection, random::Random, round_robin::RoundRobin,
    sticky_cookie::StickyCookie, LoadBalancingStrategy,
  },
  middleware::{
    authentication::Authentication, compression::Compression, custom_error_pages::CustomErrorPages,
    https_redirector::HttpsRedirector, maxbodysize::MaxBodySize, rate_limiter::RateLimiter, Middleware,
    MiddlewareChain,
  },
  server::{BackendPool, BackendPoolBuilder, Scheme, SharedData},
  tls::{certified_key_from_acme_certificate, load_certified_key},
};
use arc_swap::ArcSwap;
use log::{info, trace, warn};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use serde::Deserialize;
use std::{
  collections::{HashMap, HashSet},
  convert::{TryFrom, TryInto},
  error::Error,
  fmt::Debug,
  fs, io,
  ops::Deref,
  path::Path,
  sync::{mpsc::channel, Arc},
  thread::spawn,
  time::Duration,
};
use tokio::sync::watch::{self};
use tokio_rustls::{rustls::sign::CertifiedKey, webpki::DNSNameRef};
use toml::{value::Table, Value};

pub async fn read_config<P: AsRef<Path>>(path: P) -> Result<Arc<ArcSwap<SharedData>>, io::Error> {
  let shared_data = read_shared_data(&path).await?;
  Ok(Arc::new(ArcSwap::from_pointee(shared_data)))
}

pub async fn watch_config<P>(path: P, config: Arc<ArcSwap<SharedData>>) -> Result<(), io::Error>
where
  P: AsRef<Path> + Send + 'static,
{
  let mut receiver = start_config_watcher(path);
  loop {
    receiver.changed().await.map_err(broken_pipe)?;

    match receiver.borrow().deref() {
      DebouncedEvent::Write(path) => match read_shared_data(&path).await {
        Ok(shared_data) => config.store(Arc::new(shared_data)),
        Err(e) => warn!("{}", e),
      },
      DebouncedEvent::Remove(path) => warn!("{} was deleted", path.display()),
      e => trace!("{:?}", e),
    }
  }
}

fn start_config_watcher<P>(path: P) -> watch::Receiver<DebouncedEvent>
where
  P: AsRef<Path> + Send + 'static,
{
  let (sender, receiver) = watch::channel(DebouncedEvent::Write(path.as_ref().into()));
  spawn(move || watch_config_blocking(path, sender));
  receiver
}

fn watch_config_blocking<P: AsRef<Path>>(
  path: P,
  async_sender: watch::Sender<DebouncedEvent>,
) -> Result<(), io::Error> {
  let (sender, receiver) = channel();
  let mut watcher = watcher(sender, Duration::from_secs(1)).map_err(map_notify_error)?;
  watcher
    .watch(path, RecursiveMode::NonRecursive)
    .map_err(map_notify_error)?;
  loop {
    let evt = receiver.recv().map_err(broken_pipe)?;
    async_sender.send(evt).map_err(broken_pipe)?;
  }
}

async fn read_shared_data<P>(path: P) -> Result<SharedData, io::Error>
where
  P: AsRef<Path>,
{
  let config = Config::read(&path)?;
  shared_data_from_config(config).await
}

async fn shared_data_from_config(other: Config) -> Result<SharedData, io::Error> {
  let acme_handler = AcmeHandler::new();
  let backend_pools = other.backend_pools.into_iter().map(|it| Arc::new(it.into())).collect();

  let mut certificates = HashMap::new();
  for (sni_name, certificate_config) in other.certificates {
    let dns_name = DNSNameRef::try_from_ascii_str(&sni_name)
      .map_err(invalid_data)?
      .to_owned();
    let certificate = create_certified_key(certificate_config, dns_name.as_ref(), &acme_handler).await?;
    certificates.insert(dns_name, certificate);
  }

  Ok(SharedData::new(backend_pools, certificates, acme_handler))
}

async fn create_certified_key(
  config: CertificateConfig,
  sni_name: DNSNameRef<'_>,
  acme_handler: &AcmeHandler,
) -> Result<CertifiedKey, io::Error> {
  let certified_key = match config {
    CertificateConfig::Local {
      certificate_path,
      private_key_path,
    } => load_certified_key(certificate_path, private_key_path)?,
    CertificateConfig::ACME {
      staging,
      email,
      alt_names,
      persist_dir,
    } => {
      // TODO refresh certificates once they expire?
      let certificate = acme_handler
        .initiate_challenge(staging, &persist_dir, &email, sni_name.into(), &alt_names)
        .await
        .map_err(other)?;

      certified_key_from_acme_certificate(certificate)?
    }
  };
  certified_key
    .cross_check_end_entity_cert(Some(sni_name))
    .map_err(invalid_data)?;
  Ok(certified_key)
}

fn map_notify_error(error: notify::Error) -> io::Error {
  match error {
    notify::Error::Generic(e) => other(e),
    notify::Error::Io(e) => e,
    notify::Error::PathNotFound => not_found(error),
    notify::Error::WatchNotFound => not_found(error),
  }
}

fn broken_pipe<E>(error: E) -> io::Error
where
  E: Into<Box<dyn Error + Send + Sync>>,
{
  io::Error::new(io::ErrorKind::BrokenPipe, error)
}

fn invalid_data<E>(error: E) -> io::Error
where
  E: Into<Box<dyn Error + Send + Sync>>,
{
  io::Error::new(io::ErrorKind::InvalidData, error)
}

fn not_found<E>(error: E) -> io::Error
where
  E: Into<Box<dyn Error + Send + Sync>>,
{
  io::Error::new(io::ErrorKind::NotFound, error)
}

fn other<E>(error: E) -> io::Error
where
  E: Into<Box<dyn Error + Send + Sync>>,
{
  io::Error::new(io::ErrorKind::Other, error)
}

#[derive(Debug, Deserialize)]
struct Config {
  backend_pools: Vec<BackendPoolConfig>,
  #[serde(default)]
  certificates: HashMap<String, CertificateConfig>,
}

impl Config {
  fn read<P: AsRef<Path>>(toml_path: P) -> io::Result<Config> {
    let toml_str = fs::read_to_string(&toml_path).map_err(|e| {
      io::Error::new(
        e.kind(),
        format!(
          "Error occurred when reading configuration file {}: {}",
          toml_path.as_ref().display(),
          e
        ),
      )
    })?;
    let config: Config = toml::from_str(&toml_str).map_err(|e| {
      let e = io::Error::from(e);
      io::Error::new(
        e.kind(),
        format!(
          "Error occurred when parsing configuration file {}: {}",
          toml_path.as_ref().display(),
          e
        ),
      )
    })?;
    info!("Successfully parsed configuration!");
    config.print_warnings();
    Ok(config)
  }

  fn print_warnings(&self) {
    for (index, pool) in self.backend_pools.iter().enumerate() {
      if pool.schemes.is_empty() {
        warn!("backend pool at index {} is unreachable, since no schemes are registered. Consider adding `HTTP` or `HTTPS` to the schemes array.", index);
      }

      if pool.addresses.is_empty() {
        warn!(
          "backend pool at index {} does not contain any addresses. It will always result in bad gateway errors.",
          index
        );
      }
    }
  }
}

#[derive(Debug, Deserialize)]
struct BackendPoolConfig {
  matcher: String,
  addresses: Vec<String>,
  schemes: HashSet<Scheme>,
  client: Option<ClientConfig>,
  health_config: HealthConfig,
  strategy: LoadBalancingStrategyConfig,
  #[serde(default)]
  middlewares: Table,
}

impl From<BackendPoolConfig> for BackendPool {
  fn from(other: BackendPoolConfig) -> Self {
    // TODO: This conversion can fail, should we use TryFrom or wrap this in some kind of error?
    let matcher = other.matcher.into();
    let addresses = other
      .addresses
      .into_iter()
      .map(|address| (address, ArcSwap::from_pointee(Healthiness::Healthy)))
      .collect();
    let health_config = other.health_config;
    let strategy = other.strategy.into();
    let chain = other.middlewares.into();
    let schemes = other.schemes;

    let mut builder = BackendPoolBuilder::new(matcher, addresses, health_config, strategy, chain, schemes);
    if let Some(client) = other.client {
      if let Some(pool_idle_timeout) = client.pool_idle_timeout {
        builder.pool_idle_timeout(pool_idle_timeout);
      }

      if let Some(pool_max_idle_per_host) = client.pool_max_idle_per_host {
        builder.pool_max_idle_per_host(pool_max_idle_per_host);
      }
    }

    builder.build()
  }
}

#[derive(Debug, Deserialize)]
struct ClientConfig {
  pool_idle_timeout: Option<Duration>,
  pool_max_idle_per_host: Option<usize>,
}

#[derive(Debug, Deserialize, PartialEq)]
enum LoadBalancingStrategyConfig {
  StickyCookie {
    cookie_name: String,
    http_only: bool,
    secure: bool,
    same_site: StickyCookieSameSite,
    inner: Box<LoadBalancingStrategyConfig>,
  },
  Random,
  IPHash,
  LeastConnection,
  RoundRobin,
}

impl From<LoadBalancingStrategyConfig> for Box<dyn LoadBalancingStrategy> {
  fn from(other: LoadBalancingStrategyConfig) -> Self {
    match other {
      LoadBalancingStrategyConfig::StickyCookie {
        cookie_name,
        http_only,
        secure,
        same_site,
        inner,
      } => {
        let inner = (*inner).into();
        Box::new(StickyCookie::new(
          cookie_name,
          inner,
          http_only,
          secure,
          same_site.into(),
        ))
      }
      LoadBalancingStrategyConfig::Random => Box::new(Random::new()),
      LoadBalancingStrategyConfig::IPHash => Box::new(IPHash::new()),
      LoadBalancingStrategyConfig::RoundRobin => Box::new(RoundRobin::new()),
      LoadBalancingStrategyConfig::LeastConnection => Box::new(LeastConnection::new()),
    }
  }
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum StickyCookieSameSite {
  Strict,
  Lax,
  None,
}

impl From<StickyCookieSameSite> for cookie::SameSite {
  fn from(other: StickyCookieSameSite) -> Self {
    match other {
      StickyCookieSameSite::Strict => cookie::SameSite::Strict,
      StickyCookieSameSite::Lax => cookie::SameSite::Lax,
      StickyCookieSameSite::None => cookie::SameSite::None,
    }
  }
}

impl From<Table> for MiddlewareChain {
  fn from(other: Table) -> Self {
    let mut chain = MiddlewareChain::Empty;
    for middleware in other.into_iter().rev() {
      if let Ok(middleware) = middleware.try_into() {
        chain = MiddlewareChain::Entry {
          middleware,
          chain: Box::new(chain),
        };
      }
    }
    chain
  }
}

impl TryFrom<(String, Value)> for Box<dyn Middleware> {
  type Error = ();

  fn try_from((name, payload): (String, Value)) -> Result<Self, Self::Error> {
    match (name.as_str(), payload) {
      ("RateLimiter", Value::Table(t)) => Ok(Box::new(RateLimiter::new(
        t.get("limit")
          .and_then(Value::as_integer)
          .and_then(|it| it.try_into().ok())
          .ok_or(())?,
        t.get("window_sec")
          .and_then(Value::as_integer)
          .and_then(|it| it.try_into().ok())
          .ok_or(())?,
      ))),
      ("Authentication", Value::Table(t)) => Ok(Box::new(Authentication {
        ldap_address: t.get("ldap_address").and_then(Value::as_str).ok_or(())?.to_string(),
        user_directory: t.get("user_directory").and_then(Value::as_str).ok_or(())?.to_string(),
        rdn_identifier: t.get("rdn_identifier").and_then(Value::as_str).ok_or(())?.to_string(),
        recursive: t.get("recursive").and_then(Value::as_bool).ok_or(())?,
      })),
      ("Compression", _) => Ok(Box::new(Compression)),
      ("HttpsRedirector", _) => Ok(Box::new(HttpsRedirector)),
      ("MaxBodySize", Value::Table(t)) => Ok(Box::new(MaxBodySize {
        limit: t.get("limit").and_then(Value::as_integer).ok_or(())?,
      })),
      ("CustomErrorPages", Value::Table(t)) => Ok(Box::new(CustomErrorPages::try_from(t)?)),
      _ => Err(()),
    }
  }
}

#[derive(Debug, Deserialize)]
pub enum CertificateConfig {
  Local {
    certificate_path: String,
    private_key_path: String,
  },
  ACME {
    staging: bool,
    email: String,
    alt_names: Vec<String>,
    persist_dir: String,
  },
}
