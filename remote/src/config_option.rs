use crate::config::Config;
use nexus_actor_core_rs::actor::core::Props;

#[derive(Debug, Clone)]
pub enum ConfigOption {
  SetHost(String),
  SetPort(u16),
  SetAdvertisedAddress(String),
  PutKind(String, Props),
  SetEndpointWriterBatchSize(usize),
  SetEndpointWriterQueueSize(usize),
  SetEndpointManagerBatchSize(usize),
  SetEndpointManagerQueueSize(usize),
}

impl ConfigOption {
  pub async fn apply(&self, config: &mut Config) {
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
      ConfigOption::PutKind(kind, props) => {
        config.put_kind(kind, props.clone()).await;
      }
      ConfigOption::SetEndpointWriterBatchSize(batch_size) => {
        config.set_endpoint_writer_batch_size(*batch_size).await;
      }
      ConfigOption::SetEndpointWriterQueueSize(queue_size) => {
        config.set_endpoint_writer_queue_size(*queue_size).await;
      }
      ConfigOption::SetEndpointManagerBatchSize(batch_size) => {
        config.set_endpoint_manager_batch_size(*batch_size).await;
      }
      ConfigOption::SetEndpointManagerQueueSize(queue_size) => {
        config.set_endpoint_manager_queue_size(*queue_size).await;
      }
    }
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

  pub fn with_kind(kind: &str, props: Props) -> ConfigOption {
    ConfigOption::PutKind(kind.to_string(), props)
  }

  pub fn with_endpoint_writer_batch_size(batch_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointWriterBatchSize(batch_size)
  }

  pub fn with_endpoint_writer_queue_size(queue_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointWriterQueueSize(queue_size)
  }

  pub fn with_endpoint_manager_batch_size(batch_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointManagerBatchSize(batch_size)
  }

  pub fn with_endpoint_manager_queue_size(queue_size: usize) -> ConfigOption {
    ConfigOption::SetEndpointManagerQueueSize(queue_size)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn config_option_apply_updates_config() {
    let mut config = Config::default();
    let options = vec![
      ConfigOption::with_host("192.168.0.1"),
      ConfigOption::with_port(9001),
      ConfigOption::with_advertised_address("my-host:9001"),
      ConfigOption::with_endpoint_writer_batch_size(5),
      ConfigOption::with_endpoint_writer_queue_size(20),
      ConfigOption::with_endpoint_manager_batch_size(13),
      ConfigOption::with_endpoint_manager_queue_size(23),
    ];

    for option in &options {
      option.apply(&mut config).await;
    }

    assert_eq!(config.get_host().await.unwrap(), "192.168.0.1");
    assert_eq!(config.get_port().await.unwrap(), 9001);
    assert_eq!(config.get_advertised_address().await.unwrap(), "my-host:9001");
    assert_eq!(config.get_endpoint_writer_batch_size().await, 5);
    assert_eq!(config.get_endpoint_writer_queue_size().await, 20);
    assert_eq!(config.get_endpoint_manager_batch_size().await, 13);
    assert_eq!(config.get_endpoint_manager_queue_size().await, 23);
  }
}
