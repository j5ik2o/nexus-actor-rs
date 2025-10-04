use crate::actor::dispatch::{CoreSchedulerDispatcher, Dispatcher};
use crate::actor::ConfigOption;
use crate::runtime::tokio_core_runtime;
use nexus_actor_core_rs::runtime::CoreRuntime;
use opentelemetry::metrics::{Meter, MeterProvider};
use opentelemetry::{global, InstrumentationScope};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub enum MetricsProvider {
  Global,
  Sdk(SdkMeterProvider),
  Custom(Arc<dyn MeterProvider + Send + Sync>),
}

impl std::fmt::Debug for MetricsProvider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      MetricsProvider::Global => write!(f, "MetricsProvider::Global"),
      MetricsProvider::Sdk(_) => write!(f, "MetricsProvider::Sdk"),
      MetricsProvider::Custom(_) => write!(f, "MetricsProvider::Custom"),
    }
  }
}

impl MeterProvider for MetricsProvider {
  fn meter(&self, name: &'static str) -> Meter {
    match self {
      MetricsProvider::Global => global::meter_provider().meter(name),
      MetricsProvider::Sdk(provider) => provider.meter(name),
      MetricsProvider::Custom(provider) => provider.meter(name),
    }
  }

  fn meter_with_scope(&self, scope: InstrumentationScope) -> Meter {
    match self {
      MetricsProvider::Global => global::meter_provider().meter_with_scope(scope),
      MetricsProvider::Sdk(provider) => provider.meter_with_scope(scope),
      MetricsProvider::Custom(provider) => provider.meter_with_scope(scope),
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
  pub mailbox_metrics_poll_interval: Duration,
  pub core_runtime: CoreRuntime,
  // Other fields...
}

impl Default for Config {
  fn default() -> Self {
    Config {
      metrics_provider: None,
      log_prefix: "".to_string(),
      system_dispatcher: Arc::new(CoreSchedulerDispatcher::from_runtime(tokio_core_runtime())),
      dispatcher_throughput: 300,
      dead_letter_throttle_interval: Duration::from_secs(1),
      dead_letter_throttle_count: 10,
      dead_letter_request_logging: false,
      developer_supervision_logging: false,
      mailbox_metrics_poll_interval: Duration::from_millis(250),
      core_runtime: tokio_core_runtime(),
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
    self.metrics_provider.as_ref().is_some()
  }
}
