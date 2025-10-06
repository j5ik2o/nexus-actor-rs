use crate::config::server_config::ServerConfig;
use crate::config_option::{ConfigOption, ConfigOptionError};
use dashmap::DashMap;
use nexus_actor_std_rs::actor::core::Props;
use nexus_remote_core_rs::TransportEndpoint;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use url::Url;
pub mod server_config;

#[derive(Debug)]
struct ConfigInner {
  host: Option<String>,
  port: Option<u16>,
  advertised_address: Option<String>,
  endpoint_writer_batch_size: usize,
  endpoint_writer_queue_size: usize,
  endpoint_writer_queue_snapshot_interval: usize,
  endpoint_manager_batch_size: usize,
  endpoint_manager_queue_size: usize,
  kinds: DashMap<String, Props>,
  max_retry_count: u32,
  retry_interval: Duration,
  server_config: Option<ServerConfig>,
  reconnect_max_retries: u32,
  reconnect_initial_backoff: Duration,
  reconnect_max_backoff: Duration,
  heartbeat_interval: Duration,
  heartbeat_timeout: Duration,
  backpressure_warning_threshold: f64,
  backpressure_critical_threshold: f64,
  initial_blocked_members: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
  inner: Arc<Mutex<ConfigInner>>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportEndpointError {
  #[error("transport endpoint missing host component: {uri}")]
  MissingHost { uri: String },
  #[error("transport endpoint missing port component: {uri}")]
  MissingPort { uri: String },
  #[error("invalid transport endpoint: {uri}")]
  Invalid { uri: String },
}

impl Default for Config {
  fn default() -> Self {
    Self {
      inner: Arc::new(Mutex::new(ConfigInner {
        host: None,
        port: None,
        advertised_address: None,
        endpoint_writer_batch_size: 1000,
        endpoint_manager_batch_size: 1000,
        endpoint_writer_queue_size: 1000000,
        // 運用推奨値に合わせて既定値を 32 に設定（dev は必要に応じて ConfigOption で上書き）
        endpoint_writer_queue_snapshot_interval: 32,
        endpoint_manager_queue_size: 1000000,
        kinds: DashMap::new(),
        max_retry_count: 5,
        retry_interval: Duration::from_secs(2),
        server_config: None,
        reconnect_max_retries: 5,
        reconnect_initial_backoff: Duration::from_millis(200),
        reconnect_max_backoff: Duration::from_secs(5),
        heartbeat_interval: Duration::from_secs(15),
        heartbeat_timeout: Duration::from_secs(45),
        backpressure_warning_threshold: 0.6,
        backpressure_critical_threshold: 0.85,
        initial_blocked_members: Vec::new(),
      })),
    }
  }
}

impl Config {
  pub async fn from(options: impl IntoIterator<Item = ConfigOption>) -> Config {
    Self::try_from_options(options)
      .await
      .unwrap_or_else(|err| panic!("invalid remote config option: {err}"))
  }

  pub async fn try_from_options(options: impl IntoIterator<Item = ConfigOption>) -> Result<Config, ConfigOptionError> {
    let options = options.into_iter().collect::<Vec<_>>();
    let mut config = Config::default();
    for option in &options {
      option.apply(&mut config).await?;
    }
    Ok(config)
  }

  pub async fn add_initial_blocked_member(&mut self, member: String) {
    let mut mg = self.inner.lock().await;
    if !mg.initial_blocked_members.iter().any(|existing| existing == &member) {
      mg.initial_blocked_members.push(member);
    }
  }

  pub async fn initial_blocked_members(&self) -> Vec<String> {
    let mg = self.inner.lock().await;
    mg.initial_blocked_members.clone()
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
    mg.port
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

  pub async fn get_advertised_address(&self) -> Option<String> {
    let mg = self.inner.lock().await;
    mg.advertised_address.clone()
  }

  pub async fn set_advertised_address(&mut self, advertised_address: String) {
    let mut mg = self.inner.lock().await;
    mg.advertised_address = Some(advertised_address);
  }

  pub async fn set_transport_endpoint(&mut self, endpoint: &TransportEndpoint) -> Result<(), TransportEndpointError> {
    let (host, port) = parse_transport_endpoint(endpoint)?;
    self.set_host(host).await;
    self.set_port(port).await;
    self.set_advertised_address(endpoint.uri.clone()).await;
    Ok(())
  }

  pub async fn transport_endpoint(&self) -> Option<TransportEndpoint> {
    if let Some(advertised) = self.get_advertised_address().await {
      return Some(TransportEndpoint::new(advertised));
    }

    match (self.get_host().await, self.get_port().await) {
      (Some(host), Some(port)) => Some(TransportEndpoint::new(format!("{host}:{port}"))),
      _ => None,
    }
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

  pub async fn get_endpoint_writer_queue_snapshot_interval(&self) -> usize {
    let mg = self.inner.lock().await;
    mg.endpoint_writer_queue_snapshot_interval
  }

  pub async fn set_endpoint_writer_queue_snapshot_interval(&mut self, endpoint_writer_queue_snapshot_interval: usize) {
    let mut mg = self.inner.lock().await;
    mg.endpoint_writer_queue_snapshot_interval = endpoint_writer_queue_snapshot_interval.max(1);
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
    mg.retry_interval
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

  pub async fn get_endpoint_reconnect_max_retries(&self) -> u32 {
    let mg = self.inner.lock().await;
    mg.reconnect_max_retries
  }

  pub async fn set_endpoint_reconnect_max_retries(&mut self, reconnect_max_retries: u32) {
    let mut mg = self.inner.lock().await;
    mg.reconnect_max_retries = reconnect_max_retries;
  }

  pub async fn get_endpoint_reconnect_initial_backoff(&self) -> Duration {
    let mg = self.inner.lock().await;
    mg.reconnect_initial_backoff
  }

  pub async fn set_endpoint_reconnect_initial_backoff(&mut self, backoff: Duration) {
    let mut mg = self.inner.lock().await;
    mg.reconnect_initial_backoff = backoff;
  }

  pub async fn get_endpoint_reconnect_max_backoff(&self) -> Duration {
    let mg = self.inner.lock().await;
    mg.reconnect_max_backoff
  }

  pub async fn set_endpoint_reconnect_max_backoff(&mut self, backoff: Duration) {
    let mut mg = self.inner.lock().await;
    mg.reconnect_max_backoff = backoff;
  }

  pub async fn get_endpoint_heartbeat_interval(&self) -> Duration {
    let mg = self.inner.lock().await;
    mg.heartbeat_interval
  }

  pub async fn set_endpoint_heartbeat_interval(&mut self, interval: Duration) {
    let mut mg = self.inner.lock().await;
    mg.heartbeat_interval = interval;
  }

  pub async fn get_endpoint_heartbeat_timeout(&self) -> Duration {
    let mg = self.inner.lock().await;
    mg.heartbeat_timeout
  }

  pub async fn set_endpoint_heartbeat_timeout(&mut self, timeout: Duration) {
    let mut mg = self.inner.lock().await;
    mg.heartbeat_timeout = timeout;
  }

  pub async fn get_backpressure_warning_threshold(&self) -> f64 {
    let mg = self.inner.lock().await;
    mg.backpressure_warning_threshold
  }

  pub async fn set_backpressure_warning_threshold(&mut self, threshold: f64) {
    let mut mg = self.inner.lock().await;
    mg.backpressure_warning_threshold = threshold;
  }

  pub async fn get_backpressure_critical_threshold(&self) -> f64 {
    let mg = self.inner.lock().await;
    mg.backpressure_critical_threshold
  }

  pub async fn set_backpressure_critical_threshold(&mut self, threshold: f64) {
    let mut mg = self.inner.lock().await;
    mg.backpressure_critical_threshold = threshold;
  }
}

pub(crate) fn parse_transport_endpoint(endpoint: &TransportEndpoint) -> Result<(String, u16), TransportEndpointError> {
  let uri = endpoint.uri.trim();
  if uri.is_empty() {
    return Err(TransportEndpointError::Invalid { uri: uri.to_string() });
  }

  if let Ok(url) = Url::parse(uri) {
    if let Some(host) = url.host_str() {
      if let Some(port) = url.port().or_else(|| url.port_or_known_default()) {
        return Ok((host.to_string(), port));
      }
    }
  }

  if let Ok(url) = Url::parse(&format!("tcp://{uri}")) {
    if let Some(host) = url.host_str() {
      if let Some(port) = url.port().or_else(|| url.port_or_known_default()) {
        return Ok((host.to_string(), port));
      }
    }
  }

  if let Ok(addr) = uri.parse::<SocketAddr>() {
    return Ok((addr.ip().to_string(), addr.port()));
  }

  if let Some((host_part, port_part)) = uri.rsplit_once(':') {
    if let Ok(port) = port_part.parse::<u16>() {
      let host = host_part.trim();
      if !host.is_empty() {
        return Ok((host.to_string(), port));
      }
    }
  }

  Err(TransportEndpointError::Invalid { uri: uri.to_string() })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::server_config::ServerConfig;
  use nexus_actor_std_rs::actor::core::Props;
  use nexus_remote_core_rs::TransportEndpoint;

  #[tokio::test]
  async fn config_accessors_cover_writer_and_manager_fields() {
    let mut config = Config::default();
    config.set_host("127.0.0.1".into()).await;
    config.set_port(8080).await;
    config.set_advertised_address("127.0.0.1:8080".into()).await;
    config.set_endpoint_writer_batch_size(5).await;
    config.set_endpoint_writer_queue_size(10).await;
    config.set_endpoint_manager_batch_size(7).await;
    config.set_endpoint_manager_queue_size(11).await;
    config.set_max_retry_count(3).await;
    config.set_retry_interval(Duration::from_millis(200)).await;
    config.set_server_config(ServerConfig::default()).await;

    let kinds = DashMap::new();
    let dummy_props = Props::from_async_actor_receiver(|_| async { Ok(()) }).await;
    kinds.insert("foo".into(), dummy_props.clone());
    config.set_kinds(kinds.clone()).await;
    config.put_kind("bar", dummy_props).await;

    assert_eq!(config.get_host().await.unwrap(), "127.0.0.1");
    assert_eq!(config.get_port().await.unwrap(), 8080);
    assert_eq!(config.get_address().await, "127.0.0.1:8080");
    assert_eq!(config.get_socket_address().await, "127.0.0.1:8080".parse().unwrap());
    assert_eq!(config.get_endpoint_writer_batch_size().await, 5);
    assert_eq!(config.get_endpoint_writer_queue_size().await, 10);
    assert_eq!(config.get_endpoint_manager_batch_size().await, 7);
    assert_eq!(config.get_endpoint_manager_queue_size().await, 11);
    assert_eq!(config.get_max_retry_count().await, 3);
    assert_eq!(config.get_retry_interval().await, Duration::from_millis(200));
    assert!(config.get_server_config().await.is_some());
    assert_eq!(config.get_kinds().await.len(), kinds.len() + 1);
  }

  #[tokio::test]
  async fn config_transport_endpoint_round_trip() {
    let mut config = Config::default();
    let endpoint = TransportEndpoint::new("tcp://127.0.0.1:8123".to_string());

    config.set_transport_endpoint(&endpoint).await.unwrap();

    assert_eq!(config.get_host().await.unwrap(), "127.0.0.1");
    assert_eq!(config.get_port().await.unwrap(), 8123);
    assert_eq!(config.transport_endpoint().await.unwrap().uri, endpoint.uri);
  }

  #[tokio::test]
  async fn config_try_from_options_accepts_transport_endpoint() {
    let endpoint = TransportEndpoint::new("127.0.0.1:9200".to_string());
    let config = Config::try_from_options([ConfigOption::with_transport_endpoint(endpoint.clone())])
      .await
      .unwrap();

    let resolved = config.transport_endpoint().await.unwrap();
    assert_eq!(resolved.uri, endpoint.uri);
  }
}
