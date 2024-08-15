use crate::actor::config::Config;
use std::time::Duration;

pub enum ConfigOption {
  // SetMetricsProvider(Arc<dyn MetricsProvider>),
  SetLogPrefix(String),
  SetDispatcherThroughput(usize),
  SetDeadLetterThrottleInterval(Duration),
  SetDeadLetterThrottleCount(usize),
  SetDeadLetterRequestLogging(bool),
  // Other options...
}

impl ConfigOption {
  pub(crate) fn apply(&self, config: &mut Config) {
    match self {
      // ConfigOption::SetMetricsProvider(provider) => {
      //   config.metrics_provider = Some(Arc::clone(provider));
      // },
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

}

