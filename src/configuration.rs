use crate::{
  health::Healthiness,
  load_balancing::{
    ip_hash::IPHash, least_connection::LeastConnection, random::Random, round_robin::RoundRobin,
    sticky_cookie::StickyCookie, LoadBalancingStrategy,
  },
  middleware::{compression::Compression, https_redirector::HttpsRedirector, Middleware, MiddlewareChain},
  server::{BackendPool, BackendPoolBuilder, Scheme, SharedData},
};
use arc_swap::ArcSwap;
use futures::Future;
use log::{error, info, warn};
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::{
  collections::{HashMap, HashSet},
  convert::{TryFrom, TryInto},
  fs,
  sync::{mpsc::channel, Arc},
  time::Duration,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use toml::{value::Table, Value};

pub struct BackendConfigWatcher {
  toml_path: String,
}

impl BackendConfigWatcher {
  pub fn new(toml_path: String) -> BackendConfigWatcher {
    BackendConfigWatcher { toml_path }
  }

  fn start_config_watcher(toml_path: &str, cs: UnboundedSender<Config>) -> RecommendedWatcher {
    let toml_path = toml_path.to_string();
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(10)).unwrap();
    watcher.watch(&toml_path, RecursiveMode::NonRecursive).unwrap();

    std::thread::spawn(move || loop {
      let option_config = match rx.recv() {
        Ok(event) => match event {
          DebouncedEvent::NoticeWrite(_) => Config::new(&toml_path),
          DebouncedEvent::Write(_) => Config::new(&toml_path),
          _ => None,
        },
        Err(_) => {
          return;
        }
      };

      if let Some(config) = option_config {
        match cs.send(config) {
          Ok(_) => {}
          Err(e) => error!(
            "Error occurred when sending backend config from config watcher thread: {:?}",
            e
          ),
        };
      }
    });
    watcher
  }

  pub async fn watch_config_and_apply<F, Fut, Out>(&mut self, task_fn: F) -> !
  where
    F: Fn(Arc<ArcSwap<SharedData>>) -> Fut,
    Fut: Future<Output = Out> + 'static + Send,
    Out: 'static + Send,
  {
    let (cs, mut cr) = unbounded_channel();
    // dropping this would stop the config watcher
    let _watcher = BackendConfigWatcher::start_config_watcher(&self.toml_path, cs);

    let initial_config = Config::new(&self.toml_path);
    let initial_config = if initial_config.is_some() {
      initial_config.unwrap()
    } else {
      cr.recv().await.unwrap()
    };

    let shared_data: SharedData = initial_config.into();
    let config = Arc::new(ArcSwap::from(Arc::new(shared_data)));
    tokio::spawn(task_fn(config.clone()));

    loop {
      let new_config = cr.recv().await.unwrap();
      config.store(Arc::new(new_config.into()));
    }
  }
}

#[derive(Debug, Deserialize)]
struct Config {
  backend_pools: Vec<BackendPoolConfig>,
  #[serde(default)]
  certificates: HashMap<String, CertificateConfig>,
}

impl Config {
  fn new(toml_path: &str) -> Option<Config> {
    let toml_str_result = fs::read_to_string(toml_path);
    let toml_str = match toml_str_result {
      Ok(toml_str) => toml_str,
      Err(e) => {
        warn!("Error occurred when reading configuration file {}: {}", toml_path, e);
        return None;
      }
    };

    let config_result: Result<Config, toml::de::Error> = toml::from_str(toml_str.as_str());
    match config_result {
      Ok(config) => {
        info!("Successfully parsed configuration!");
        config.print_warnings();
        Some(config)
      }
      Err(e) => {
        warn!("Error occurred when parsing configuration file {}: {}", toml_path, e);
        None
      }
    }
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
    SharedData {
      backend_pools,
      certificates,
    }
  }
}

#[derive(Debug, Deserialize)]
struct BackendPoolConfig {
  matcher: String,
  addresses: Vec<String>,
  schemes: HashSet<Scheme>,
  client: Option<ClientConfig>,
  strategy: LoadBalancingStrategyConfig,
  #[serde(default)]
  middlewares: Table,
}

impl From<BackendPoolConfig> for BackendPool {
  fn from(other: BackendPoolConfig) -> Self {
    // TODO: This conversion can fail, should we use TryFrom or wrap this in some kind of error?
    let matcher = other.matcher.into();
    let addresses: Vec<(String, Arc<ArcSwap<Healthiness>>)> = other
      .addresses
      .into_iter()
      .map(|address| (address, Arc::new(ArcSwap::from_pointee(Healthiness::Healthy))))
      .collect();
    let strategy = other.strategy.into();
    let chain = other.middlewares.into();
    let schemes = other.schemes;

    let mut builder = BackendPoolBuilder::new(matcher, addresses, strategy, chain, schemes);
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

  fn try_from((name, _payload): (String, Value)) -> Result<Self, Self::Error> {
    match name.as_str() {
      "Compression" => Ok(Box::new(Compression)),
      "HttpsRedirector" => Ok(Box::new(HttpsRedirector)),
      _ => Err(()),
    }
  }
}

#[derive(Debug, Deserialize)]
pub struct CertificateConfig {
  pub certificate_path: String,
  pub private_key_path: String,
}
