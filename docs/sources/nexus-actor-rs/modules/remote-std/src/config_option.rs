use crate::config::{Config, TransportEndpointError};
use nexus_actor_std_rs::actor::core::Props;
use nexus_remote_core_rs::TransportEndpoint;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum ConfigOption {
  SetHost(String),
  SetPort(u16),
  SetAdvertisedAddress(String),
  SetTransportEndpoint(TransportEndpoint),
  PutKind(String, Props),
  SetEndpointWriterBatchSize(usize),
  SetEndpointWriterQueueSize(usize),
  SetEndpointWriterQueueSnapshotInterval(usize),
  SetEndpointManagerBatchSize(usize),
  SetEndpointManagerQueueSize(usize),
  SetEndpointReconnectMaxRetries(u32),
  SetEndpointReconnectInitialBackoff(Duration),
  SetEndpointReconnectMaxBackoff(Duration),
  SetEndpointHeartbeatInterval(Duration),
  SetEndpointHeartbeatTimeout(Duration),
  SetBackpressureWarningThreshold(f64),
  SetBackpressureCriticalThreshold(f64),
  AddInitialBlockedMember(String),
}

#[derive(Debug, Error)]
pub enum ConfigOptionError {
  #[error(transparent)]
  TransportEndpoint(#[from] TransportEndpointError),
}

impl ConfigOption {
  pub async fn apply(&self, config: &mut Config) -> Result<(), ConfigOptionError> {
    match self {
      ConfigOption::SetHost(host) => {
        config.set_host(host.clone()).await;
      }
      ConfigOption::SetPort(port) => {
        config.set_port(*port).await;
      }
      ConfigOption::SetAdvertisedAddress(advertised_address) => {
        config.set_advertised_address(advertised_address.clone()).await;
      }
      ConfigOption::SetTransportEndpoint(endpoint) => {
        config.set_transport_endpoint(endpoint).await?;
      }
      ConfigOption::PutKind(kind, props) => {
        config.put_kind(kind, props.clone()).await;
      }
      ConfigOption::SetEndpointWriterBatchSize(batch_size) => {
        config.set_endpoint_writer_batch_size(*batch_size).await;
      }
      ConfigOption::SetEndpointWriterQueueSize(queue_size) => {
        config.set_endpoint_writer_queue_size(*queue_size).await;
      }
      ConfigOption::SetEndpointWriterQueueSnapshotInterval(interval) => {
        config.set_endpoint_writer_queue_snapshot_interval(*interval).await;
      }
      ConfigOption::SetEndpointManagerBatchSize(batch_size) => {
        config.set_endpoint_manager_batch_size(*batch_size).await;
      }
      ConfigOption::SetEndpointManagerQueueSize(queue_size) => {
        config.set_endpoint_manager_queue_size(*queue_size).await;
      }
      ConfigOption::SetEndpointReconnectMaxRetries(max_retries) => {
        config.set_endpoint_reconnect_max_retries(*max_retries).await;
      }
      ConfigOption::SetEndpointReconnectInitialBackoff(backoff) => {
        config.set_endpoint_reconnect_initial_backoff(*backoff).await;
      }
      ConfigOption::SetEndpointReconnectMaxBackoff(backoff) => {
        config.set_endpoint_reconnect_max_backoff(*backoff).await;
      }
      ConfigOption::SetEndpointHeartbeatInterval(interval) => {
        config.set_endpoint_heartbeat_interval(*interval).await;
      }
      ConfigOption::SetEndpointHeartbeatTimeout(timeout) => {
        config.set_endpoint_heartbeat_timeout(*timeout).await;
      }
      ConfigOption::SetBackpressureWarningThreshold(threshold) => {
        config.set_backpressure_warning_threshold(*threshold).await;
      }
      ConfigOption::SetBackpressureCriticalThreshold(threshold) => {
        config.set_backpressure_critical_threshold(*threshold).await;
      }
      ConfigOption::AddInitialBlockedMember(member) => {
        config.add_initial_blocked_member(member.clone()).await;
      }
    }
    Ok(())
  }

  pub fn with_host(host: &str) -> ConfigOption {
    ConfigOption::SetHost(host.to_string())
  }

  pub fn with_port(port: u16) -> ConfigOption {
    ConfigOption::SetPort(port)
  }

  pub fn with_advertised_address(advertised_address: &str) -> ConfigOption {
    ConfigOption::SetAdvertisedAddress(advertised_address.to_string())
  }

  pub fn with_transport_endpoint(endpoint: TransportEndpoint) -> ConfigOption {
    ConfigOption::SetTransportEndpoint(endpoint)
  }

  pub fn with_kind(kind: &str, props: Props) -> ConfigOption {
    ConfigOption::PutKind(kind.to_string(), props)
  }

  pub fn with_endpoint_writer_batch_size(batch_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointWriterBatchSize(batch_size)
  }

  pub fn with_endpoint_writer_queue_size(queue_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointWriterQueueSize(queue_size)
  }

  pub fn with_endpoint_writer_queue_snapshot_interval(interval: usize) -> ConfigOption {
    ConfigOption::SetEndpointWriterQueueSnapshotInterval(interval)
  }

  pub fn with_endpoint_manager_batch_size(batch_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointManagerBatchSize(batch_size)
  }

  pub fn with_endpoint_manager_queue_size(queue_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointManagerQueueSize(queue_size)
  }

  pub fn with_endpoint_reconnect_max_retries(max_retries: u32) -> ConfigOption {
    ConfigOption::SetEndpointReconnectMaxRetries(max_retries)
  }

  pub fn with_endpoint_reconnect_initial_backoff(backoff: Duration) -> ConfigOption {
    ConfigOption::SetEndpointReconnectInitialBackoff(backoff)
  }

  pub fn with_endpoint_reconnect_max_backoff(backoff: Duration) -> ConfigOption {
    ConfigOption::SetEndpointReconnectMaxBackoff(backoff)
  }

  pub fn with_endpoint_heartbeat_interval(interval: Duration) -> ConfigOption {
    ConfigOption::SetEndpointHeartbeatInterval(interval)
  }

  pub fn with_endpoint_heartbeat_timeout(timeout: Duration) -> ConfigOption {
    ConfigOption::SetEndpointHeartbeatTimeout(timeout)
  }

  pub fn with_backpressure_warning_threshold(threshold: f64) -> ConfigOption {
    ConfigOption::SetBackpressureWarningThreshold(threshold)
  }

  pub fn with_backpressure_critical_threshold(threshold: f64) -> ConfigOption {
    ConfigOption::SetBackpressureCriticalThreshold(threshold)
  }

  pub fn with_initial_blocked_member(member: impl Into<String>) -> ConfigOption {
    ConfigOption::AddInitialBlockedMember(member.into())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_remote_core_rs::TransportEndpoint;

  #[tokio::test]
  async fn config_option_apply_updates_config() {
    let mut config = Config::default();
    let options = vec![
      ConfigOption::with_host("192.168.0.1"),
      ConfigOption::with_port(9001),
      ConfigOption::with_advertised_address("my-host:9001"),
      ConfigOption::with_endpoint_writer_batch_size(5),
      ConfigOption::with_endpoint_writer_queue_size(20),
      ConfigOption::with_endpoint_writer_queue_snapshot_interval(7),
      ConfigOption::with_endpoint_manager_batch_size(13),
      ConfigOption::with_endpoint_manager_queue_size(23),
      ConfigOption::with_endpoint_reconnect_max_retries(8),
      ConfigOption::with_endpoint_reconnect_initial_backoff(Duration::from_millis(500)),
      ConfigOption::with_endpoint_reconnect_max_backoff(Duration::from_secs(8)),
      ConfigOption::with_endpoint_heartbeat_interval(Duration::from_secs(10)),
      ConfigOption::with_endpoint_heartbeat_timeout(Duration::from_secs(30)),
      ConfigOption::with_backpressure_warning_threshold(0.4),
      ConfigOption::with_backpressure_critical_threshold(0.75),
      ConfigOption::with_initial_blocked_member("node-initial"),
    ];

    for option in &options {
      option.apply(&mut config).await.unwrap();
    }

    assert_eq!(config.get_host().await.unwrap(), "192.168.0.1");
    assert_eq!(config.get_port().await.unwrap(), 9001);
    assert_eq!(config.get_advertised_address().await.unwrap(), "my-host:9001");
    assert_eq!(config.get_endpoint_writer_batch_size().await, 5);
    assert_eq!(config.get_endpoint_writer_queue_size().await, 20);
    assert_eq!(config.get_endpoint_writer_queue_snapshot_interval().await, 7);
    assert_eq!(config.get_endpoint_manager_batch_size().await, 13);
    assert_eq!(config.get_endpoint_manager_queue_size().await, 23);
    assert_eq!(config.get_endpoint_reconnect_max_retries().await, 8);
    assert_eq!(
      config.get_endpoint_reconnect_initial_backoff().await,
      Duration::from_millis(500)
    );
    assert_eq!(
      config.get_endpoint_reconnect_max_backoff().await,
      Duration::from_secs(8)
    );
    assert_eq!(config.get_endpoint_heartbeat_interval().await, Duration::from_secs(10));
    assert_eq!(config.get_endpoint_heartbeat_timeout().await, Duration::from_secs(30));
    assert_eq!(config.get_backpressure_warning_threshold().await, 0.4);
    assert_eq!(config.get_backpressure_critical_threshold().await, 0.75);
    assert_eq!(config.initial_blocked_members().await, vec!["node-initial".to_string()]);
  }

  #[tokio::test]
  async fn config_option_transport_endpoint_sets_fields() {
    let mut config = Config::default();
    let endpoint = TransportEndpoint::new("tcp://10.1.2.3:7001".to_string());

    ConfigOption::with_transport_endpoint(endpoint.clone())
      .apply(&mut config)
      .await
      .unwrap();

    assert_eq!(config.get_host().await.unwrap(), "10.1.2.3");
    assert_eq!(config.get_port().await.unwrap(), 7001);
    assert_eq!(config.transport_endpoint().await.unwrap().uri, endpoint.uri);
  }
}
