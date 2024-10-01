use crate::config::server_config::ServerConfig;
use crate::config_option::ConfigOption;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::actor::Props;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
pub mod server_config;

#[derive(Debug)]
struct ConfigInner {
  host: Option<String>,
  port: Option<u16>,
  advertised_host: Option<String>,
  endpoint_writer_batch_size: usize,
  endpoint_writer_queue_size: usize,
  endpoint_manager_batch_size: usize,
  endpoint_manager_queue_size: usize,
  kinds: DashMap<String, Props>,
  max_retry_count: u32,
  retry_interval: Duration,
  server_config: Option<ServerConfig>,
}

#[derive(Debug, Clone)]
pub struct Config {
  inner: Arc<Mutex<ConfigInner>>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      inner: Arc::new(Mutex::new(ConfigInner {
        host: None,
        port: None,
        advertised_host: None,
        endpoint_writer_batch_size: 1000,
        endpoint_manager_batch_size: 1000,
        endpoint_writer_queue_size: 1000000,
        endpoint_manager_queue_size: 1000000,
        kinds: DashMap::new(),
        max_retry_count: 5,
        retry_interval: Duration::from_secs(2),
        server_config: None,
      })),
    }
  }
}

impl Config {
  pub async fn from(options: impl IntoIterator<Item = ConfigOption>) -> Config {
    let options = options.into_iter().collect::<Vec<_>>();
    let mut config = Config::default();
    for option in options {
      option.apply(&mut config).await;
    }
    config
  }

  pub async fn get_host(&self) -> Option<String> {
    let mg = self.inner.lock().await;
    mg.host.clone()
  }

  pub async fn set_host(&mut self, host: String) {
    let mut mg = self.inner.lock().await;
    mg.host = Some(host);
  }

  pub async fn get_port(&self) -> Option<u16> {
    let mg = self.inner.lock().await;
    mg.port.clone()
  }

  pub async fn set_port(&mut self, port: u16) {
    let mut mg = self.inner.lock().await;
    mg.port = Some(port);
  }

  pub async fn get_address(&self) -> String {
    match (&self.get_host().await, &self.get_port().await) {
      (Some(host), Some(port)) => format!("{}:{}", host, port),
      _ => panic!("Host and port must be set"),
    }
  }

  pub async fn get_socket_address(&self) -> SocketAddr {
    let host = self.get_host().await;
    let port = self.get_port().await;

    match (host, port) {
      (Some(host), Some(port)) => {
        let host = IpAddr::from_str(&host).expect("Invalid host");
        SocketAddr::from((host, port))
      }
      _ => panic!("Host and port must be set"),
    }
  }

  pub async fn get_advertised_host(&self) -> Option<String> {
    let mg = self.inner.lock().await;
    mg.advertised_host.clone()
  }

  pub async fn set_advertised_host(&mut self, advertised_host: String) {
    let mut mg = self.inner.lock().await;
    mg.advertised_host = Some(advertised_host);
  }

  pub async fn get_endpoint_writer_batch_size(&self) -> usize {
    let mg = self.inner.lock().await;
    mg.endpoint_writer_batch_size
  }

  pub async fn set_endpoint_writer_batch_size(&mut self, endpoint_writer_batch_size: usize) {
    let mut mg = self.inner.lock().await;
    mg.endpoint_writer_batch_size = endpoint_writer_batch_size;
  }

  pub async fn get_endpoint_writer_queue_size(&self) -> usize {
    let mg = self.inner.lock().await;
    mg.endpoint_writer_queue_size
  }

  pub async fn set_endpoint_writer_queue_size(&mut self, endpoint_writer_queue_size: usize) {
    let mut mg = self.inner.lock().await;
    mg.endpoint_writer_queue_size = endpoint_writer_queue_size;
  }

  pub async fn get_endpoint_manager_batch_size(&self) -> usize {
    let mg = self.inner.lock().await;
    mg.endpoint_manager_batch_size
  }

  pub async fn set_endpoint_manager_batch_size(&mut self, endpoint_manager_batch_size: usize) {
    let mut mg = self.inner.lock().await;
    mg.endpoint_manager_batch_size = endpoint_manager_batch_size;
  }

  pub async fn get_endpoint_manager_queue_size(&self) -> usize {
    let mg = self.inner.lock().await;
    mg.endpoint_manager_queue_size
  }

  pub async fn set_endpoint_manager_queue_size(&mut self, endpoint_manager_queue_size: usize) {
    let mut mg = self.inner.lock().await;
    mg.endpoint_manager_queue_size = endpoint_manager_queue_size;
  }

  pub async fn get_max_retry_count(&self) -> u32 {
    let mg = self.inner.lock().await;
    mg.max_retry_count
  }

  pub async fn set_max_retry_count(&mut self, max_retry_count: u32) {
    let mut mg = self.inner.lock().await;
    mg.max_retry_count = max_retry_count;
  }

  pub async fn get_kinds(&self) -> DashMap<String, Props> {
    let mg = self.inner.lock().await;
    mg.kinds.clone()
  }

  pub async fn set_kinds(&mut self, kinds: DashMap<String, Props>) {
    let mut mg = self.inner.lock().await;
    mg.kinds = kinds;
  }

  pub async fn put_kind(&mut self, kind: &str, props: Props) {
    let mg = self.inner.lock().await;
    mg.kinds.insert(kind.to_string(), props);
  }

  pub async fn get_retry_interval(&self) -> Duration {
    let mg = self.inner.lock().await;
    mg.retry_interval.clone()
  }

  pub async fn set_retry_interval(&mut self, retry_interval: Duration) {
    let mut mg = self.inner.lock().await;
    mg.retry_interval = retry_interval;
  }

  pub async fn get_server_config(&self) -> Option<ServerConfig> {
    let mg = self.inner.lock().await;
    mg.server_config.clone()
  }

  pub async fn set_server_config(&mut self, server_config: ServerConfig) {
    let mut mg = self.inner.lock().await;
    mg.server_config = Some(server_config);
  }
}
