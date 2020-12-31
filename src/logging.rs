use log::{info, LevelFilter};
use log4rs::{
  append::console::ConsoleAppender,
  config::{Appender, Root},
  Config,
};

pub fn initialize() -> log4rs::Handle {
  let log_level = std::env::var("LOG_LEVEL").unwrap_or("DEBUG".into());
  let level_filter = parse_level_filter(&log_level).expect(&format!("Invalid log level: {}", &log_level));

  let stdout = ConsoleAppender::builder().build();
  let config = Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .build(Root::builder().appender("stdout").build(level_filter))
    .unwrap();

  let handle = log4rs::init_config(config).expect("Initializing logging should not fail");
  info!("Logging Level: {}", &level_filter);
  handle
}

fn parse_level_filter(str: &str) -> Option<LevelFilter> {
  match str.to_lowercase().as_str() {
    "off" => Some(LevelFilter::Off),
    "error" => Some(LevelFilter::Error),
    "warn" => Some(LevelFilter::Warn),
    "info" => Some(LevelFilter::Info),
    "debug" => Some(LevelFilter::Debug),
    "trace" => Some(LevelFilter::Trace),
    _ => None,
  }
}
