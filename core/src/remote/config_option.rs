use crate::remote::config::Config;

#[derive(Debug, Clone)]
pub enum ConfigOption {
  SetHost(String),
  SetPort(u16),
  SetAdvertisedHost(String),
}

impl ConfigOption {
  pub fn apply(&self, config: &mut Config) {
    match self {
      ConfigOption::SetHost(host) => {
        config.host = Some(host.clone());
      }
      ConfigOption::SetPort(port) => {
        config.port = Some(*port);
      }
      ConfigOption::SetAdvertisedHost(advertised_host) => {
        config.advertised_host = Some(advertised_host.clone());
      }
    }
  }

  pub fn with_host(host: String) -> ConfigOption {
    ConfigOption::SetHost(host)
  }

  pub fn with_port(port: u16) -> ConfigOption {
    ConfigOption::SetPort(port)
  }

  pub fn with_advertised_host(advertised_host: String) -> ConfigOption {
    ConfigOption::SetAdvertisedHost(advertised_host)
  }
}
