use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
  Connected,
  Suspended,
  Reconnecting,
  Closed,
}

impl ConnectionState {
  fn as_u8(self) -> u8 {
    match self {
      ConnectionState::Connected => 0,
      ConnectionState::Suspended => 1,
      ConnectionState::Reconnecting => 2,
      ConnectionState::Closed => 3,
    }
  }

  fn from_u8(value: u8) -> ConnectionState {
    match value {
      0 => ConnectionState::Connected,
      1 => ConnectionState::Suspended,
      2 => ConnectionState::Reconnecting,
      _ => ConnectionState::Closed,
    }
  }
}

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
  max_retries: u32,
  initial_backoff: Duration,
  max_backoff: Duration,
}

impl ReconnectPolicy {
  pub fn new(max_retries: u32, initial_backoff: Duration, max_backoff: Duration) -> Self {
    Self {
      max_retries,
      initial_backoff,
      max_backoff,
    }
  }

  pub fn max_retries(&self) -> u32 {
    self.max_retries
  }

  pub fn initial_backoff(&self) -> Duration {
    self.initial_backoff
  }

  pub fn max_backoff(&self) -> Duration {
    self.max_backoff
  }
}

#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
  interval: Duration,
  timeout: Duration,
}

impl HeartbeatConfig {
  pub fn new(interval: Duration, timeout: Duration) -> Self {
    Self { interval, timeout }
  }

  pub fn interval(&self) -> Duration {
    self.interval
  }

  pub fn timeout(&self) -> Duration {
    self.timeout
  }
}

#[derive(Debug)]
pub struct EndpointState {
  address: String,
  connection_state: AtomicU8,
  reconnect_policy: ReconnectPolicy,
  heartbeat: HeartbeatConfig,
  retry_count: AtomicU32,
}

impl EndpointState {
  pub fn new(address: impl Into<String>, reconnect_policy: ReconnectPolicy, heartbeat: HeartbeatConfig) -> Self {
    Self {
      address: address.into(),
      connection_state: AtomicU8::new(ConnectionState::Suspended.as_u8()),
      reconnect_policy,
      heartbeat,
      retry_count: AtomicU32::new(0),
    }
  }

  pub fn address(&self) -> &str {
    &self.address
  }

  pub fn connection_state(&self) -> ConnectionState {
    ConnectionState::from_u8(self.connection_state.load(Ordering::SeqCst))
  }

  pub fn set_connection_state(&self, state: ConnectionState) {
    self.connection_state.store(state.as_u8(), Ordering::SeqCst);
  }

  pub fn reconnect_policy(&self) -> &ReconnectPolicy {
    &self.reconnect_policy
  }

  pub fn heartbeat_config(&self) -> &HeartbeatConfig {
    &self.heartbeat
  }

  pub fn retries(&self) -> u32 {
    self.retry_count.load(Ordering::SeqCst)
  }

  pub fn reset_retries(&self) {
    self.retry_count.store(0, Ordering::SeqCst);
  }

  pub fn record_retry_attempt(&self) -> u32 {
    self.retry_count.fetch_add(1, Ordering::SeqCst) + 1
  }

  pub fn has_exceeded_retries(&self) -> bool {
    let max = self.reconnect_policy.max_retries();
    max > 0 && self.retries() >= max
  }

  pub fn next_backoff_delay(&self) -> Duration {
    self.compute_backoff_delay(self.retries().saturating_add(1))
  }

  pub fn compute_backoff_delay(&self, attempts: u32) -> Duration {
    if attempts <= 1 {
      return self.reconnect_policy.initial_backoff();
    }

    let mut delay = self.reconnect_policy.initial_backoff();
    for _ in 1..attempts {
      delay = delay.checked_mul(2).unwrap_or(Duration::MAX);
      if delay >= self.reconnect_policy.max_backoff() {
        delay = self.reconnect_policy.max_backoff();
        break;
      }
    }

    if delay > self.reconnect_policy.max_backoff() {
      self.reconnect_policy.max_backoff()
    } else {
      delay
    }
  }
}

pub type EndpointStateHandle = Arc<EndpointState>;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn backoff_doubles_until_cap() {
    let policy = ReconnectPolicy::new(5, Duration::from_millis(200), Duration::from_secs(3));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(15), Duration::from_secs(45));
    let state = EndpointState::new("node-A", policy, heartbeat);

    assert_eq!(state.connection_state(), ConnectionState::Suspended);

    let delays: Vec<_> = (1..=5).map(|attempt| state.compute_backoff_delay(attempt)).collect();

    assert_eq!(delays[0], Duration::from_millis(200));
    assert_eq!(delays[1], Duration::from_millis(400));
    assert_eq!(delays[2], Duration::from_millis(800));
    assert_eq!(delays[3], Duration::from_millis(1600));
    assert_eq!(delays[4], Duration::from_secs(3));
  }

  #[test]
  fn retry_tracking_and_limits() {
    let policy = ReconnectPolicy::new(2, Duration::from_millis(100), Duration::from_secs(1));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(10), Duration::from_secs(30));
    let state = EndpointState::new("node-B", policy, heartbeat);

    assert_eq!(state.retries(), 0);
    assert!(!state.has_exceeded_retries());

    state.record_retry_attempt();
    assert_eq!(state.retries(), 1);
    assert!(!state.has_exceeded_retries());

    state.record_retry_attempt();
    assert_eq!(state.retries(), 2);
    assert!(state.has_exceeded_retries());

    state.reset_retries();
    assert_eq!(state.retries(), 0);
    assert!(!state.has_exceeded_retries());
  }

  #[test]
  fn connection_state_transitions() {
    let policy = ReconnectPolicy::new(0, Duration::from_secs(1), Duration::from_secs(1));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(3));
    let state = EndpointState::new("node-C", policy, heartbeat);

    state.set_connection_state(ConnectionState::Connected);
    assert_eq!(state.connection_state(), ConnectionState::Connected);

    state.set_connection_state(ConnectionState::Reconnecting);
    assert_eq!(state.connection_state(), ConnectionState::Reconnecting);

    state.set_connection_state(ConnectionState::Closed);
    assert_eq!(state.connection_state(), ConnectionState::Closed);
  }
}
