use crate::{
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
};
use arc_swap::ArcSwap;
use log::{info, trace, warn};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use serde::Deserialize;
use std::{
  collections::{HashMap, HashSet},
  convert::{TryFrom, TryInto},
  fmt::Debug,
  fs, io,
  path::Path,
  sync::{
    mpsc::{channel, RecvError},
    Arc,
  },
  thread::spawn,
  time::Duration,
};
use toml::{value::Table, Value};

pub async fn read_config<P: AsRef<Path>>(path: P) -> Result<Arc<ArcSwap<SharedData>>, io::Error> {
  let config = Config::read(&path)?;
  Ok(Arc::new(ArcSwap::from_pointee(config.into())))
}

pub fn start_config_watcher<P>(path: P, config: Arc<ArcSwap<SharedData>>)
where
  P: AsRef<Path> + Send + 'static,
{
  spawn(move || update_config_on_file_change(path, config));
}

fn update_config_on_file_change<P: AsRef<Path>>(path: P, config: Arc<ArcSwap<SharedData>>) -> Result<(), io::Error> {
  let (tx, rx) = channel();
  let mut watcher = watcher(tx, Duration::from_secs(1)).map_err(map_notify_error)?;
  watcher
    .watch(path, RecursiveMode::NonRecursive)
    .map_err(map_notify_error)?;

  loop {
    match rx.recv().map_err(map_recv_error)? {
      DebouncedEvent::Write(path) => match Config::read(&path) {
        Ok(new_config) => config.store(Arc::new(new_config.into())),
        Err(e) => warn!("{}", e),
      },
      DebouncedEvent::Remove(path) => warn!("{} was deleted", path.display()),
      e => trace!("{:?}", e),
    }
  }
}

fn map_notify_error(error: notify::Error) -> io::Error {
  match error {
    notify::Error::Generic(e) => io::Error::new(io::ErrorKind::Other, e),
    notify::Error::Io(e) => e,
    notify::Error::PathNotFound => io::Error::new(io::ErrorKind::NotFound, error),
    notify::Error::WatchNotFound => io::Error::new(io::ErrorKind::NotFound, error),
  }
}

fn map_recv_error(error: RecvError) -> io::Error {
  io::Error::new(io::ErrorKind::BrokenPipe, error)
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

impl From<Config> for SharedData {
  fn from(other: Config) -> Self {
    let certificates = other.certificates;
    let backend_pools = other.backend_pools.into_iter().map(|b| Arc::new(b.into())).collect();
    SharedData::new(backend_pools, certificates)
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
