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
}
