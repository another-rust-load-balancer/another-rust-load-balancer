use crate::server::SharedData;
use arc_swap::ArcSwap;
use hyper::{
  client::HttpConnector,
  http::uri::{self, Authority},
  Client, StatusCode, Uri,
};
use hyper_timeout::TimeoutConnector;
use log::info;
use std::convert::TryFrom;
use std::time::Duration;
use std::time::SystemTime;
use std::{fmt, sync::Arc};

// Amount of time in seconds to pass until the next health check is started
const CHECK_INTERVAL: i64 = 60;
// Threshold for the response time in milliseconds when a successful health check is marked Healthy::Slow
const SLOW_THRESHOLD: i64 = 100;
//  Timeout in milliseconds when the health check fails
const TIMEOUT: u64 = 500;

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
      Healthiness::Slow(respone_time) => write!(f, "Slow {}", respone_time),
      Healthiness::Unresponsive(Some(status_code)) => write!(f, "Unresponsive, status: {}", status_code),
      Healthiness::Unresponsive(None) => write!(f, "Unresponsive"),
    }
  }
}

pub async fn watch_health(shared_data: Arc<ArcSwap<SharedData>>) {
  let mut interval_timer = tokio::time::interval(chrono::Duration::seconds(CHECK_INTERVAL).to_std().unwrap());
  loop {
    interval_timer.tick().await;
    let data_copy = shared_data.clone();
    tokio::spawn(async move {
      check_health_once(data_copy).await;
    });
  }
}

async fn check_health_once(shared_data: Arc<ArcSwap<SharedData>>) {
  let data = shared_data.load();

  for pool in &data.backend_pools {
    for (server_address, healthiness) in &pool.addresses {
      let uri = uri::Uri::builder()
        .scheme("http")
        .path_and_query("/")
        .authority(Authority::from_maybe_shared(server_address.clone()).unwrap())
        .build()
        .unwrap();

      let previous_healthiness = healthiness.load();
      let result = contact_server(uri).await;

      if previous_healthiness.as_ref() != &result {
        info!("new healthiness for {}: {}", &server_address, &result);
        healthiness.store(Arc::new(result));
      }
    }
  }
}

async fn contact_server(server_address: Uri) -> Healthiness {
  let http_connector = HttpConnector::new();
  let mut connector = TimeoutConnector::new(http_connector);
  connector.set_connect_timeout(Some(Duration::from_millis(TIMEOUT)));
  connector.set_read_timeout(Some(Duration::from_millis(TIMEOUT)));
  connector.set_write_timeout(Some(Duration::from_millis(TIMEOUT)));
  let client = Client::builder().build::<_, hyper::Body>(connector);

  let before_request = SystemTime::now();
  // Await the response...
  if let Ok(response) = client.get(server_address).await {
    if response.status().is_success() {
      // elapsed() only fails when system time is later than "self"
      let time_to_respond = before_request.elapsed().unwrap().as_millis();
      let response_time = i64::try_from(time_to_respond);
      if response_time.unwrap() > SLOW_THRESHOLD {
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
