use crate::server::BackendPool;
use arc_swap::{access::Access, ArcSwap};
use futures::future::join_all;
use hyper::{
  client::HttpConnector,
  http::uri::{self, Authority},
  Client, StatusCode, Uri,
};
use hyper_timeout::TimeoutConnector;
use log::info;
use serde::Deserialize;
use std::time::Duration;
use std::time::SystemTime;
use std::{convert::TryFrom, ops::Deref};
use std::{fmt, sync::Arc};
use tokio::time::interval;
/* Contains the user preferences regarding health checks */
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct HealthConfig {
  pub slow_threshold: i64,
  pub timeout: u64,
  pub path: String,
}
/* Healthiness of a backend server */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Healthiness {
  Healthy,
  Slow(i64),
  Unresponsive(Option<StatusCode>),
}

impl fmt::Display for Healthiness {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Healthiness::Healthy => write!(f, "Healthy"),
      Healthiness::Slow(response_time) => write!(f, "Slow {}", response_time),
      Healthiness::Unresponsive(Some(status_code)) => write!(f, "Unresponsive, status: {}", status_code),
      Healthiness::Unresponsive(None) => write!(f, "Unresponsive"),
    }
  }
}
/* Start loop to regularly contact backend to investigate the healthiness of each server.
The healthiness is noted in the backend_pool vector  */
pub async fn watch_health<A, G, H, J>(backend_pools: A, interval_duration: H)
where
  A: Access<Vec<Arc<BackendPool>>, Guard = G> + Send + Sync + 'static,
  G: Deref<Target = Vec<Arc<BackendPool>>> + Send + Sync,
  H: Access<Duration, Guard = J>,
  J: Deref<Target = Duration>,
{
  loop {
    // set and start interval timer
    let interval_duration = *interval_duration.load().deref();
    if interval_duration == Duration::from_secs(0) {
      tokio::time::sleep(Duration::from_secs(5)).await;
      continue;
    }
    let mut interval_timer = interval(interval_duration);
    interval_timer.tick().await;
    // create and perform server checks
    let loaded_pools = backend_pools.load();
    let mut checks = Vec::new();
    for pool in loaded_pools.iter() {
      for (server_address, healthiness) in &pool.addresses {
        let future = check_server_health_once(server_address.clone(), healthiness, &pool.health_config);
        checks.push(future);
      }
    }
    join_all(checks).await;
    /*  Yes tick is called twice in one loop on purpose.
    Since we are recreating interval_timer on every loop,
    the first tick, marks the starting point, resuming immediately.
    */
    interval_timer.tick().await;
  }
}
/* Contacts one server and sets health value if changed */
async fn check_server_health_once(
  server_address: String,
  healthiness: &ArcSwap<Healthiness>,
  health_config: &HealthConfig,
) {
  let uri = uri::Uri::builder()
    .scheme("http")
    .path_and_query(&health_config.path)
    .authority(Authority::from_maybe_shared(server_address.clone()).unwrap())
    .build()
    .unwrap();

  let previous_healthiness = healthiness.load();
  let result = contact_server(uri, health_config.slow_threshold, health_config.timeout).await;

  if previous_healthiness.as_ref() != &result {
    info!("new healthiness for {}: {}", &server_address, &result);
    healthiness.store(Arc::new(result));
  }
}
/* Returns the healthiness of the given server by performing a network request  */
async fn contact_server(server_address: Uri, slow_threshold: i64, timeout: u64) -> Healthiness {
  let http_connector = HttpConnector::new();
  let mut connector = TimeoutConnector::new(http_connector);
  connector.set_connect_timeout(Some(Duration::from_millis(timeout)));
  connector.set_read_timeout(Some(Duration::from_millis(timeout)));
  connector.set_write_timeout(Some(Duration::from_millis(timeout)));
  let client = Client::builder().build::<_, hyper::Body>(connector);

  let before_request = SystemTime::now();
  // Await the response...
  if let Ok(response) = client.get(server_address).await {
    if response.status().is_success() {
      // elapsed() only fails when system time is later than "self"
      let time_to_respond = before_request.elapsed().unwrap().as_millis();
      let response_time = i64::try_from(time_to_respond);
      if response_time.unwrap() > slow_threshold {
        Healthiness::Slow(response_time.unwrap())
      } else {
        Healthiness::Healthy
      }
    } else {
      Healthiness::Unresponsive(Some(response.status()))
    }
  } else {
    Healthiness::Unresponsive(None)
  }
}
