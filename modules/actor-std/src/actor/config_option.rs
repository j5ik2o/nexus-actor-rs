use crate::actor::config::Config;
use crate::actor::dispatch::Dispatcher;
use crate::actor::MetricsProvider;
use nexus_actor_core_rs::runtime::CoreRuntime;
use std::sync::Arc;
use std::time::Duration;

pub enum ConfigOption {
  SetMetricsProvider(Arc<MetricsProvider>),
  SetLogPrefix(String),
  SetSystemDispatcher(Arc<dyn Dispatcher>),
  SetDispatcherThroughput(usize),
  SetDeadLetterThrottleInterval(Duration),
  SetDeadLetterThrottleCount(usize),
  SetDeadLetterRequestLogging(bool),
  SetMailboxMetricsPollInterval(Duration),
  SetCoreRuntime(CoreRuntime),
  // Other options...
}

impl ConfigOption {
  pub(crate) fn apply(&self, config: &mut Config) {
    match self {
      ConfigOption::SetMetricsProvider(provider) => {
        config.metrics_provider = Some(Arc::clone(provider));
      }
      ConfigOption::SetSystemDispatcher(dispatcher) => {
        config.system_dispatcher = Arc::clone(dispatcher);
      }
      ConfigOption::SetLogPrefix(prefix) => {
        config.log_prefix = prefix.clone();
      }
      ConfigOption::SetDispatcherThroughput(throughput) => {
        config.dispatcher_throughput = *throughput;
      }
      ConfigOption::SetDeadLetterThrottleInterval(interval) => {
        config.dead_letter_throttle_interval = *interval;
      }
      ConfigOption::SetDeadLetterThrottleCount(count) => {
        config.dead_letter_throttle_count = *count;
      }
      ConfigOption::SetDeadLetterRequestLogging(enabled) => {
        config.dead_letter_request_logging = *enabled;
      }
      ConfigOption::SetMailboxMetricsPollInterval(interval) => {
        config.mailbox_metrics_poll_interval = *interval;
      }
      ConfigOption::SetCoreRuntime(runtime) => {
        config.core_runtime = runtime.clone();
      } // Handle other options...
    }
  }

  pub fn with_dead_letter_throttle_interval(duration: Duration) -> ConfigOption {
    ConfigOption::SetDeadLetterThrottleInterval(duration)
  }

  pub fn with_dead_letter_throttle_count(count: usize) -> ConfigOption {
    ConfigOption::SetDeadLetterThrottleCount(count)
  }

  pub fn with_dead_letter_request_logging(enabled: bool) -> ConfigOption {
    ConfigOption::SetDeadLetterRequestLogging(enabled)
  }

  pub fn with_mailbox_metrics_poll_interval(duration: Duration) -> ConfigOption {
    ConfigOption::SetMailboxMetricsPollInterval(duration)
  }

  pub fn with_core_runtime(runtime: CoreRuntime) -> ConfigOption {
    ConfigOption::SetCoreRuntime(runtime)
  }
}
