use crate::actor::ConfigOption;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
  // pub metrics_provider: Option<Arc<dyn MetricsProvider>>,
  pub log_prefix: String,
  pub dispatcher_throughput: usize,
  pub dead_letter_throttle_interval: Duration,
  pub dead_letter_throttle_count: usize,
  pub dead_letter_request_logging: bool,
  pub developer_supervision_logging: bool,
  // Other fields...
}

impl Default for Config {
  fn default() -> Self {
    Config {
      // metrics_provider: None,
      log_prefix: "".to_string(),
      dispatcher_throughput: 300,
      dead_letter_throttle_interval: Duration::from_secs(1),
      dead_letter_throttle_count: 10,
      dead_letter_request_logging: false,
      developer_supervision_logging: false,
      // Set other default values...
    }
  }
}

impl Config {
  pub fn from(options: impl IntoIterator<Item = ConfigOption>) -> Config {
    let options = options.into_iter().collect::<Vec<_>>();
    let mut config = Config::default();
    for option in options {
      option.apply(&mut config);
    }
    config
  }
}
