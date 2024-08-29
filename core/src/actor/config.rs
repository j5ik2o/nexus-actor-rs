use crate::actor::dispatch::{Dispatcher, TokioRuntimeContextDispatcher};
use crate::actor::ConfigOption;
use opentelemetry::global::GlobalMeterProvider;
use opentelemetry::metrics::noop::NoopMeterProvider;
use opentelemetry::metrics::{Meter, MeterProvider};
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub enum MetricsProvider {
  Global(GlobalMeterProvider),
  Noop(NoopMeterProvider),
  Sdk(SdkMeterProvider),
}

impl Clone for MetricsProvider {
  fn clone(&self) -> Self {
    match self {
      MetricsProvider::Global(provider) => MetricsProvider::Global(provider.clone()),
      MetricsProvider::Noop(_) => MetricsProvider::Noop(NoopMeterProvider::default()),
      MetricsProvider::Sdk(provider) => MetricsProvider::Sdk(provider.clone()),
    }
  }
}

impl MeterProvider for MetricsProvider {
  fn meter(&self, name: impl Into<Cow<'static, str>>) -> Meter {
    match self {
      MetricsProvider::Global(provider) => provider.meter(name),
      MetricsProvider::Noop(provider) => provider.meter(name),
      MetricsProvider::Sdk(provider) => provider.meter(name),
    }
  }

  fn versioned_meter(
    &self,
    name: impl Into<Cow<'static, str>>,
    version: Option<impl Into<Cow<'static, str>>>,
    schema_url: Option<impl Into<Cow<'static, str>>>,
    attributes: Option<Vec<KeyValue>>,
  ) -> Meter {
    match self {
      MetricsProvider::Global(provider) => provider.versioned_meter(name, version, schema_url, attributes),
      MetricsProvider::Noop(provider) => provider.versioned_meter(name, version, schema_url, attributes),
      MetricsProvider::Sdk(provider) => provider.versioned_meter(name, version, schema_url, attributes),
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
