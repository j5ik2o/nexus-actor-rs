use crate::actor::actor::Props;
use crate::remote::config::server_config::ServerConfig;
use crate::remote::config_option::ConfigOption;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::str::FromStr;

pub mod server_config;

#[derive(Debug, Clone)]
pub struct Config {
  pub host: Option<String>,
  pub port: Option<u16>,
  pub advertised_host: Option<String>,
  pub endpoint_writer_batch_size: u32,
  pub endpoint_writer_queue_size: u32,
  pub endpoint_manager_batch_size: u32,
  pub endpoint_manager_queue_size: u32,
  pub kinds: DashMap<String, Props>,
  pub max_retry_count: u32,
  pub server_config: Option<ServerConfig>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      host: None,
      port: None,
      advertised_host: None,
      endpoint_writer_batch_size: 1000,
      endpoint_manager_batch_size: 1000,
      endpoint_writer_queue_size: 1000000,
      endpoint_manager_queue_size: 1000000,
      kinds: DashMap::new(),
      max_retry_count: 5,
      server_config: None,
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

  pub fn get_address(&self) -> String {
    match (&self.host, &self.port) {
      (Some(host), Some(port)) => format!("{}:{}", host, port),
      _ => panic!("Host and port must be set"),
    }
  }

  pub fn get_socket_address(&self) -> SocketAddr {
    SocketAddr::from_str(&self.get_address()).expect("Invalid address")
  }
}
