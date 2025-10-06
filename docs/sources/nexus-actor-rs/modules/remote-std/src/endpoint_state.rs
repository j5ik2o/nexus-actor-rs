use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

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
  #[cfg_attr(not(test), allow(dead_code))]
  address: String,
  connection_state: AtomicU8,
  reconnect_policy: ReconnectPolicy,
  heartbeat: HeartbeatConfig,
  retry_count: AtomicU32,
  heartbeat_base: Instant,
  last_heartbeat_millis: AtomicU64,
  state_change: Notify,
}

impl EndpointState {
  pub fn new(address: impl Into<String>, reconnect_policy: ReconnectPolicy, heartbeat: HeartbeatConfig) -> Self {
    let now = Instant::now();
    let state = Self {
      address: address.into(),
      connection_state: AtomicU8::new(ConnectionState::Suspended.as_u8()),
      reconnect_policy,
      heartbeat,
      retry_count: AtomicU32::new(0),
      heartbeat_base: now,
      last_heartbeat_millis: AtomicU64::new(0),
      state_change: Notify::new(),
    };
    state.mark_heartbeat(now);
    state
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub fn address(&self) -> &str {
    &self.address
  }

  pub fn connection_state(&self) -> ConnectionState {
    ConnectionState::from_u8(self.connection_state.load(Ordering::SeqCst))
  }

  pub fn set_connection_state(&self, state: ConnectionState) {
    let previous = self.connection_state.swap(state.as_u8(), Ordering::SeqCst);
    if previous != state.as_u8() {
      self.state_change.notify_waiters();
    }
  }

  pub fn reconnect_policy(&self) -> &ReconnectPolicy {
    &self.reconnect_policy
  }

  pub fn heartbeat_config(&self) -> &HeartbeatConfig {
    &self.heartbeat
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub fn retries(&self) -> u32 {
    self.retry_count.load(Ordering::SeqCst)
  }

  pub fn reset_retries(&self) {
    self.retry_count.store(0, Ordering::SeqCst);
  }

  pub fn record_retry_attempt(&self) -> u32 {
    self.retry_count.fetch_add(1, Ordering::SeqCst) + 1
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub fn has_exceeded_retries(&self) -> bool {
    let max = self.reconnect_policy.max_retries();
    max > 0 && self.retries() >= max
  }

  /// Returns the backoff delay for the next scheduled reconnect attempt.
  ///
  /// This mirrors protoactor-go where the retry counter is incremented before
  /// scheduling the attempt. The current retry count therefore maps directly
  /// to the attempt number whose delay we need to compute.
  #[cfg_attr(not(test), allow(dead_code))]
  pub fn next_backoff_delay(&self) -> Duration {
    let attempts = self.retries().max(1);
    self.compute_backoff_delay(attempts)
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

  pub fn mark_heartbeat(&self, instant: Instant) {
    let elapsed = instant.checked_duration_since(self.heartbeat_base).unwrap_or_default();
    let millis = elapsed.as_millis().min(u64::MAX as u128) as u64;
    self.last_heartbeat_millis.store(millis, Ordering::SeqCst);
  }

  pub fn elapsed_since_last_heartbeat(&self, instant: Instant) -> Duration {
    let elapsed = instant.checked_duration_since(self.heartbeat_base).unwrap_or_default();
    let current_millis = elapsed.as_millis().min(u64::MAX as u128) as u64;
    let last_millis = self.last_heartbeat_millis.load(Ordering::SeqCst);
    if current_millis >= last_millis {
      Duration::from_millis(current_millis - last_millis)
    } else {
      Duration::from_millis(0)
    }
  }

  pub async fn wait_until<F>(&self, mut predicate: F) -> ConnectionState
  where
    F: FnMut(ConnectionState) -> bool, {
    loop {
      let current = self.connection_state();
      if predicate(current) {
        return current;
      }
      self.state_change.notified().await;
    }
  }

  pub async fn wait_until_connected_or_closed(&self) -> ConnectionState {
    self
      .wait_until(|state| matches!(state, ConnectionState::Connected | ConnectionState::Closed))
      .await
  }
}

pub type EndpointStateHandle = Arc<EndpointState>;

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Arc;
  use tokio::time::{sleep, Duration as TokioDuration};

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
  fn next_backoff_delay_follows_retry_count() {
    let policy = ReconnectPolicy::new(10, Duration::from_millis(100), Duration::from_secs(1));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(3));
    let state = EndpointState::new("node-delay", policy, heartbeat);

    assert_eq!(state.retries(), 0);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(100));

    let first_attempt = state.record_retry_attempt();
    assert_eq!(first_attempt, 1);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(100));

    let second_attempt = state.record_retry_attempt();
    assert_eq!(second_attempt, 2);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(200));
  }

  #[test]
  fn next_backoff_delay_respects_max_backoff() {
    let policy = ReconnectPolicy::new(10, Duration::from_millis(250), Duration::from_millis(800));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(3));
    let state = EndpointState::new("node-delay-cap", policy, heartbeat);

    assert_eq!(state.next_backoff_delay(), Duration::from_millis(250));

    let attempt_one = state.record_retry_attempt();
    assert_eq!(attempt_one, 1);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(250));

    let attempt_two = state.record_retry_attempt();
    assert_eq!(attempt_two, 2);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(500));

    let attempt_three = state.record_retry_attempt();
    assert_eq!(attempt_three, 3);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(800));

    let attempt_four = state.record_retry_attempt();
    assert_eq!(attempt_four, 4);
    assert_eq!(state.next_backoff_delay(), Duration::from_millis(800));
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

  #[test]
  fn heartbeat_tracking_updates_elapsed() {
    let policy = ReconnectPolicy::new(0, Duration::from_secs(1), Duration::from_secs(1));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(3));
    let state = EndpointState::new("node-D", policy, heartbeat);

    let base = state.heartbeat_base;
    state.mark_heartbeat(base + Duration::from_millis(250));
    let elapsed = state.elapsed_since_last_heartbeat(base + Duration::from_millis(700));
    assert_eq!(elapsed, Duration::from_millis(450));

    state.mark_heartbeat(base + Duration::from_millis(1_200));
    let elapsed_after = state.elapsed_since_last_heartbeat(base + Duration::from_millis(1_500));
    assert_eq!(elapsed_after, Duration::from_millis(300));
  }

  #[tokio::test]
  async fn wait_until_connected_or_closed_wakes_on_transition() {
    let policy = ReconnectPolicy::new(5, Duration::from_millis(200), Duration::from_secs(3));
    let heartbeat = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(3));
    let state = Arc::new(EndpointState::new("node-E", policy, heartbeat));

    let waiter = {
      let state = state.clone();
      async move { state.wait_until_connected_or_closed().await }
    };

    let setter = async {
      sleep(TokioDuration::from_millis(10)).await;
      state.set_connection_state(ConnectionState::Connected);
    };

    let (_, result) = tokio::join!(setter, waiter);
    assert_eq!(result, ConnectionState::Connected);
  }
}
