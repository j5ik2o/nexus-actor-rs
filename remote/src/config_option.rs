use crate::config::Config;
use nexus_actor_core_rs::actor::core::Props;

#[derive(Debug, Clone)]
pub enum ConfigOption {
  SetHost(String),
  SetPort(u16),
  SetAdvertisedHost(String),
  PutKind(String, Props),
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
      ConfigOption::SetAdvertisedHost(advertised_host) => {
        config.set_advertised_host(advertised_host.clone()).await;
      }
      ConfigOption::PutKind(kind, props) => {
        config.put_kind(kind, props.clone()).await;
      }
    }
  }

  pub fn with_host(host: &str) -> ConfigOption {
    ConfigOption::SetHost(host.to_string())
  }

  pub fn with_port(port: u16) -> ConfigOption {
    ConfigOption::SetPort(port)
  }

  pub fn with_advertised_host(advertised_host: &str) -> ConfigOption {
    ConfigOption::SetAdvertisedHost(advertised_host.to_string())
  }

  pub fn with_kind(kind: &str, props: Props) -> ConfigOption {
    ConfigOption::PutKind(kind.to_string(), props)
  }
}
