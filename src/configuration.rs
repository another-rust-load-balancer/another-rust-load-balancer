use crate::load_balancing::round_robin::RoundRobin;
use crate::load_balancing::sticky_cookie::StickyCookie;
use crate::load_balancing::LoadBalancingStrategy;
use crate::load_balancing::{ip_hash::IPHash, least_connection::LeastConnection};
use crate::middleware::compression::Compression;
use crate::middleware::{RequestHandler, RequestHandlerChain};
use crate::server::{BackendPool, BackendPoolConfig, SharedData};
use crate::{load_balancing::random::Random, server::BackendPoolBuilder};
use futures::Future;
use log::{error, info, warn};
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::fs;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub enum BackendConfigProtocol {
  Http {},
  Https {
    certificate_path: String,
    private_key_path: String,
  },
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum StickyCookieSameSite {
  Strict,
  Lax,
  None,
}

#[derive(Debug, Deserialize, PartialEq)]
enum BackendConfigMiddleware {
  Compression {},
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
  Random {},
  IPHash {},
  LeastConnection {},
  RoundRobin {},
}

#[derive(Debug, Deserialize)]
struct BackendConfigEntry {
  addresses: Vec<String>,
  host: String,
  strategy: BackendConfigLBStrategy,
  protocol: BackendConfigProtocol,
  chain: Vec<BackendConfigMiddleware>,
  client: Option<BackendClientConfig>,
}

#[derive(Debug, Deserialize)]
struct BackendClientConfig {
  pool_idle_timeout: Option<Duration>,
  pool_max_idle_per_host: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct BackendConfig {
  backend_pools: Vec<BackendConfigEntry>,
}

impl From<BackendConfigProtocol> for BackendPoolConfig {
  fn from(other: BackendConfigProtocol) -> Self {
    match other {
      BackendConfigProtocol::Http {} => BackendPoolConfig::HttpConfig {},
      BackendConfigProtocol::Https {
        certificate_path,
        private_key_path,
      } => BackendPoolConfig::HttpsConfig {
        certificate_path,
        private_key_path,
      },
    }
  }
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

impl From<BackendConfigMiddleware> for Box<dyn RequestHandler> {
  fn from(other: BackendConfigMiddleware) -> Self {
    match other {
      BackendConfigMiddleware::Compression { .. } => Box::new(Compression {}),
    }
  }
}

impl From<Vec<BackendConfigMiddleware>> for RequestHandlerChain {
  fn from(other: Vec<BackendConfigMiddleware>) -> Self {
    let mut chain = Box::new(RequestHandlerChain::Empty);
    for middleware in other.into_iter().rev() {
      chain = Box::new(RequestHandlerChain::Entry {
        handler: middleware.into(),
        next: chain,
      });
    }
    *chain
  }
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

impl From<BackendConfigEntry> for BackendPool {
  fn from(other: BackendConfigEntry) -> Self {
    let host = other.host;
    let addresses = other.addresses;
    let strategy = other.strategy.into();
    let config = other.protocol.into();
    let chain = other.chain.into();
    let client = other.client;

    let mut builder = BackendPoolBuilder::new(host, addresses, strategy, config, chain);
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

pub struct BackendConfigWatcher {
  toml_path: String,
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

  pub async fn watch_config_and_apply<F, Fut, Out>(&mut self, task_fn: F) -> Option<Out>
  where
    F: Fn(Arc<SharedData>) -> Fut,
    Fut: Future<Output = Out> + 'static + Send,
  {
    let (cs, mut cr) = unbounded_channel();
    // dropping this will stop the config watcher
    let _watcher = BackendConfigWatcher::start_config_watcher(&self.toml_path, cs);

    let initial_config = BackendConfig::new(&self.toml_path);
    let initial_config = if initial_config.is_some() {
      initial_config
    } else {
      cr.recv().await
    }?;

    let mut config = Arc::new(initial_config.into());
    loop {
      tokio::select! {
        r = task_fn(config) => {
          return Some(r);
        },
        c = cr.recv() => {
          let new_config = c?;
          config = Arc::new(new_config.into());
        }
      }
    }
  }
}
