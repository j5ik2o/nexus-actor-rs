use crate::actor::dispatch::{Dispatcher, TokioRuntimeContextDispatcher};
use crate::actor::ConfigOption;
use opentelemetry::global;
use opentelemetry::metrics::{Meter, MeterProvider};
use opentelemetry_sdk::metrics::MeterProvider as SdkMeterProvider;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub enum MetricsProvider {
  Noop,
  Sdk(SdkMeterProvider),
}

impl Clone for MetricsProvider {
  fn clone(&self) -> Self {
    match self {
      MetricsProvider::Sdk(provider) => MetricsProvider::Sdk(provider.clone()),
      MetricsProvider::Noop => MetricsProvider::Noop,
    }
  }
}

impl MeterProvider for MetricsProvider {
  fn meter(&self, name: &'static str) -> Meter {
    match self {
      MetricsProvider::Noop => global::meter("noop"),
      MetricsProvider::Sdk(provider) => provider.meter(name),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Config {
  pub metrics_provider: Option<Arc<MetricsProvider>>,
  pub log_prefix: String,
  pub system_dispatcher: Arc<dyn Dispatcher>,
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
      metrics_provider: None,
      log_prefix: "".to_string(),
      system_dispatcher: Arc::new(TokioRuntimeContextDispatcher::new().unwrap()),
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

  pub fn is_metrics_enabled(&self) -> bool {
    if let Some(_) = self.metrics_provider.as_ref() {
      true
    } else {
      false
    }
  }
}
