use crate::{
  health::Healthiness,
  load_balancing::{
    ip_hash::IPHash, least_connection::LeastConnection, random::Random, round_robin::RoundRobin,
    sticky_cookie::StickyCookie, LoadBalancingStrategy,
  },
  middleware::{compression::Compression, https_redirector::HttpsRedirector, Middleware, MiddlewareChain},
  server::{BackendPool, BackendPoolBuilder, BackendPoolConfig, SharedData},
};
use arc_swap::ArcSwap;
use futures::Future;
use log::{error, info, warn};
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::{
  fs,
  sync::{mpsc::channel, Arc},
  time::Duration,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct BackendConfigWatcher {
  toml_path: String,
}

impl BackendConfigWatcher {
  pub fn new(toml_path: String) -> BackendConfigWatcher {
    BackendConfigWatcher { toml_path }
  }

  fn start_config_watcher(toml_path: &str, cs: UnboundedSender<BackendConfig>) -> RecommendedWatcher {
    let toml_path = toml_path.to_string();
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(10)).unwrap();
    watcher.watch(&toml_path, RecursiveMode::NonRecursive).unwrap();

    std::thread::spawn(move || loop {
      let option_config = match rx.recv() {
        Ok(event) => match event {
          DebouncedEvent::NoticeWrite(_) => BackendConfig::new(&toml_path),
          DebouncedEvent::Write(_) => BackendConfig::new(&toml_path),
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

    let initial_config = BackendConfig::new(&self.toml_path);
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
struct BackendConfig {
  backend_pools: Vec<BackendConfigEntry>,
}

impl BackendConfig {
  fn new(toml_path: &str) -> Option<BackendConfig> {
    let toml_str_result = fs::read_to_string(toml_path);
    let toml_str = match toml_str_result {
      Ok(toml_str) => toml_str,
      Err(e) => {
        warn!("Error occurred when reading configuration file {}: {}", toml_path, e);
        return None;
      }
    };

    let config_result: Result<BackendConfig, toml::de::Error> = toml::from_str(toml_str.as_str());
    match config_result {
      Ok(config) => {
        info!("Successfully parsed configuration!");
        Some(config)
      }
      Err(e) => {
        warn!("Error occurred when parsing configuration file {}: {}", toml_path, e);
        None
      }
    }
  }
}

impl From<BackendConfig> for SharedData {
  fn from(other: BackendConfig) -> Self {
    let backend_pools = other
      .backend_pools
      .into_iter()
      .map(|b| Arc::new(b.into()))
      .collect::<Vec<Arc<BackendPool>>>();
    SharedData { backend_pools }
  }
}

#[derive(Debug, Deserialize)]
struct BackendConfigEntry {
  addresses: Vec<String>,
  matcher: String,
  strategy: BackendConfigLBStrategy,
  protocol: BackendConfigProtocol,
  chain: Vec<BackendConfigMiddleware>,
  client: Option<BackendClientConfig>,
}

impl From<BackendConfigEntry> for BackendPool {
  fn from(other: BackendConfigEntry) -> Self {
    // TODO: This conversion can fail, should we use TryFrom or wrap this in some kind of error?
    let matcher = other.matcher.into();
    let addresses: Vec<(String, Arc<ArcSwap<Healthiness>>)> = other
      .addresses
      .into_iter()
      .map(|address| (address, Arc::new(ArcSwap::from_pointee(Healthiness::Healthy))))
      .collect();
    let strategy = other.strategy.into();
    let config = other.protocol.into();
    let chain = other.chain.into();
    let client = other.client;

    let mut builder = BackendPoolBuilder::new(matcher, addresses, strategy, config, chain);
    if let Some(client) = client {
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

#[derive(Debug, Deserialize, PartialEq)]
enum BackendConfigLBStrategy {
  StickyCookie {
    cookie_name: String,
    http_only: bool,
    secure: bool,
    same_site: StickyCookieSameSite,
    inner: Box<BackendConfigLBStrategy>,
  },
  Random,
  IPHash,
  LeastConnection,
  RoundRobin,
}

impl From<BackendConfigLBStrategy> for Box<dyn LoadBalancingStrategy> {
  fn from(other: BackendConfigLBStrategy) -> Self {
    match other {
      BackendConfigLBStrategy::StickyCookie {
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
      BackendConfigLBStrategy::Random {} => Box::new(Random::new()),
      BackendConfigLBStrategy::IPHash {} => Box::new(IPHash::new()),
      BackendConfigLBStrategy::RoundRobin {} => Box::new(RoundRobin::new()),
      BackendConfigLBStrategy::LeastConnection {} => Box::new(LeastConnection::new()),
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

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub enum BackendConfigProtocol {
  Http {},
  Https {
    host: String,
    certificate_path: String,
    private_key_path: String,
  },
}

impl From<BackendConfigProtocol> for BackendPoolConfig {
  fn from(other: BackendConfigProtocol) -> Self {
    match other {
      BackendConfigProtocol::Http {} => BackendPoolConfig::HttpConfig {},
      BackendConfigProtocol::Https {
        host,
        certificate_path,
        private_key_path,
      } => BackendPoolConfig::HttpsConfig {
        host,
        certificate_path,
        private_key_path,
      },
    }
  }
}

#[derive(Debug, Deserialize, PartialEq)]
enum BackendConfigMiddleware {
  Compression,
  HttpsRedirector,
}

impl From<BackendConfigMiddleware> for Box<dyn Middleware> {
  fn from(other: BackendConfigMiddleware) -> Self {
    match other {
      BackendConfigMiddleware::Compression => Box::new(Compression {}),
      BackendConfigMiddleware::HttpsRedirector => Box::new(HttpsRedirector {}),
    }
  }
}

impl From<Vec<BackendConfigMiddleware>> for MiddlewareChain {
  fn from(other: Vec<BackendConfigMiddleware>) -> Self {
    let mut chain = Box::new(MiddlewareChain::Empty);
    for middleware in other.into_iter().rev() {
      chain = Box::new(MiddlewareChain::Entry {
        middleware: middleware.into(),
        chain,
      });
    }
    *chain
  }
}

#[derive(Debug, Deserialize)]
struct BackendClientConfig {
  pool_idle_timeout: Option<Duration>,
  pool_max_idle_per_host: Option<usize>,
}
