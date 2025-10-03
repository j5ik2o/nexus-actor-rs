use crate::activator_actor::Activator;
use crate::endpoint::Endpoint;
use crate::endpoint_lazy::EndpointLazy;
use crate::endpoint_state::{ConnectionState, EndpointState, EndpointStateHandle, HeartbeatConfig, ReconnectPolicy};
use crate::endpoint_supervisor::EndpointSupervisor;
use crate::generated::remote::RemoteMessage;
use crate::messages::{
  BackpressureLevel, EndpointConnectedEvent, EndpointEvent, EndpointReconnectEvent, EndpointTerminatedEvent,
  EndpointThrottledEvent, EndpointWatchEvent, Ping, Pong, RemoteDeliver, RemoteTerminate, RemoteUnwatch, RemoteWatch,
  WatchAction,
};
use crate::remote::Remote;
use crate::watch_registry::WatchRegistry;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use nexus_actor_core_rs::runtime::{CoreJoinHandle, CoreTaskFuture};
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{ExtendedPid, Props, SpawnError};
use nexus_actor_std_rs::actor::dispatch::{DeadLetterEvent, DispatcherHandle, SingleWorkerDispatcher};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::actor::metrics::metrics_impl::MetricsSink;
use nexus_actor_std_rs::actor::process::future::ActorFutureError;
use nexus_actor_std_rs::actor::supervisor::{RestartingStrategy, SupervisorStrategyHandle};
use nexus_actor_std_rs::event_stream::{EventHandler, Predicate, Subscription};
use nexus_actor_std_rs::generated::actor::Pid;
use nexus_utils_std_rs::collections::DashMapExtension;
use opentelemetry::KeyValue;
use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, sleep, MissedTickBehavior};
use tonic::{Request, Status, Streaming};

type ClientResponseSender = Sender<Result<RemoteMessage, Status>>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointManagerError {
  #[error("Failed to start activator: {0}")]
  ActivatorStarted(SpawnError),
  #[error("Failed to start supervisor: {0}")]
  SupervisorStarted(SpawnError),
  #[error("Failed to stop activator: {0}")]
  ActivatorStopped(ActorFutureError),
  #[error("Failed to stop supervisor: {0}")]
  SupervisorStopped(ActorFutureError),
  #[error("Failed to wait for activator: {0}")]
  Waiting(String),
}

#[derive(Debug, Clone)]
pub(crate) struct RequestKeyWrapper {
  value: Arc<Mutex<Request<Streaming<RemoteMessage>>>>,
}

impl RequestKeyWrapper {
  pub fn new(value: Arc<Mutex<Request<Streaming<RemoteMessage>>>>) -> Self {
    RequestKeyWrapper { value }
  }
}

impl PartialEq for RequestKeyWrapper {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.value, &other.value)
  }
}

impl Eq for RequestKeyWrapper {}

impl Hash for RequestKeyWrapper {
  fn hash<H: Hasher>(&self, state: &mut H) {
    Arc::as_ptr(&self.value).hash(state)
  }
}

#[derive(Debug)]
pub struct EndpointStatistics {
  queue_capacity: AtomicUsize,
  queue_size: AtomicUsize,
  dead_letters: AtomicU64,
  backpressure_level: AtomicU8,
  deliver_success: AtomicU64,
  deliver_failure: AtomicU64,
  reconnect_attempts: AtomicU64,
}

impl EndpointStatistics {
  fn new() -> Self {
    Self {
      queue_capacity: AtomicUsize::new(0),
      queue_size: AtomicUsize::new(0),
      dead_letters: AtomicU64::new(0),
      backpressure_level: AtomicU8::new(BackpressureLevel::Normal as u8),
      deliver_success: AtomicU64::new(0),
      deliver_failure: AtomicU64::new(0),
      reconnect_attempts: AtomicU64::new(0),
    }
  }

  fn snapshot(&self) -> EndpointStatisticsSnapshot {
    EndpointStatisticsSnapshot {
      queue_capacity: self.queue_capacity.load(Ordering::SeqCst),
      queue_size: self.queue_size.load(Ordering::SeqCst),
      dead_letters: self.dead_letters.load(Ordering::SeqCst),
      backpressure_level: BackpressureLevel::from_u8(self.backpressure_level.load(Ordering::SeqCst)),
      deliver_success: self.deliver_success.load(Ordering::SeqCst),
      deliver_failure: self.deliver_failure.load(Ordering::SeqCst),
      reconnect_attempts: self.reconnect_attempts.load(Ordering::SeqCst),
    }
  }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EndpointStatisticsSnapshot {
  pub queue_capacity: usize,
  pub queue_size: usize,
  pub dead_letters: u64,
  pub backpressure_level: BackpressureLevel,
  pub deliver_success: u64,
  pub deliver_failure: u64,
  pub reconnect_attempts: u64,
}

#[derive(Clone)]
pub(crate) struct EndpointManager {
  connections: Arc<DashMap<String, EndpointLazy>>,
  remote: Weak<Remote>,
  endpoint_subscription: Arc<Mutex<Option<Subscription>>>,
  endpoint_supervisor: Arc<Mutex<Option<Pid>>>,
  activator_pid: Arc<Mutex<Option<Pid>>>,
  stopped: Arc<AtomicBool>,
  #[allow(clippy::type_complexity)]
  endpoint_reader_connections: Arc<DashMap<RequestKeyWrapper, Arc<Mutex<Option<mpsc::Sender<bool>>>>>>,
  client_connections: Arc<DashMap<String, ClientResponseSender>>,
  client_connection_keys: Arc<DashMap<RequestKeyWrapper, String>>,
  endpoint_stats: Arc<DashMap<String, Arc<EndpointStatistics>>>,
  endpoint_states: Arc<DashMap<String, EndpointStateHandle>>,
  heartbeat_tasks: Arc<DashMap<String, Arc<dyn CoreJoinHandle>>>,
  watch_registries: Arc<DashMap<String, Arc<WatchRegistry>>>,
}

impl fmt::Debug for EndpointManager {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("EndpointManager").finish_non_exhaustive()
  }
}

impl EndpointManager {
  pub fn new(remote: Weak<Remote>) -> Self {
    EndpointManager {
      connections: Arc::new(DashMap::new()),
      remote,
      endpoint_subscription: Arc::new(Mutex::new(None)),
      endpoint_supervisor: Arc::new(Mutex::new(None)),
      activator_pid: Arc::new(Mutex::new(None)),
      stopped: Arc::new(AtomicBool::new(false)),
      endpoint_reader_connections: Arc::new(DashMap::new()),
      client_connections: Arc::new(DashMap::new()),
      client_connection_keys: Arc::new(DashMap::new()),
      endpoint_stats: Arc::new(DashMap::new()),
      endpoint_states: Arc::new(DashMap::new()),
      heartbeat_tasks: Arc::new(DashMap::new()),
      watch_registries: Arc::new(DashMap::new()),
    }
  }

  fn stats_entry(&self, address: &str) -> Arc<EndpointStatistics> {
    self
      .endpoint_stats
      .entry(address.to_string())
      .or_insert_with(|| Arc::new(EndpointStatistics::new()))
      .clone()
  }

  pub(crate) fn register_watch_registry(&self, address: &str, registry: Arc<WatchRegistry>) {
    self.watch_registries.insert(address.to_string(), registry);
  }

  pub(crate) fn unregister_watch_registry(&self, address: &str) {
    self.watch_registries.remove(address);
  }

  pub(crate) fn watch_registry(&self, address: &str) -> Option<Arc<WatchRegistry>> {
    self.watch_registries.get(address).map(|entry| entry.value().clone())
  }

  async fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    let remote = self.remote.upgrade()?;
    remote.metrics_sink().await
  }

  pub(crate) async fn record_queue_state(&self, address: &str, capacity: usize, len: usize) {
    let stats = self.stats_entry(address);
    stats.queue_capacity.store(capacity, Ordering::SeqCst);
    stats.queue_size.store(len, Ordering::SeqCst);

    let level = self.determine_backpressure_level(address, capacity, len).await;
    let previous = BackpressureLevel::from_u8(stats.backpressure_level.swap(level as u8, Ordering::SeqCst));

    if previous != level {
      self.publish_backpressure_event(address, level).await;
    }

    if let Some(sink) = self.metrics_sink().await {
      let labels = vec![
        KeyValue::new("remote.endpoint", address.to_string()),
        KeyValue::new("remote.queue_capacity", capacity as i64),
        KeyValue::new("remote.backpressure_level", level as i64),
      ];
      sink.record_mailbox_length_with_labels(len as u64, &labels);
    }
  }

  pub(crate) async fn increment_dead_letter(&self, address: &str) {
    let stats = self.stats_entry(address);
    stats.dead_letters.fetch_add(1, Ordering::SeqCst);
    if let Some(sink) = self.metrics_sink().await {
      let labels = vec![
        KeyValue::new("remote.endpoint", address.to_string()),
        KeyValue::new("remote.event", "dead_letter"),
      ];
      sink.increment_dead_letter_with_labels(&labels);
    }
  }

  pub(crate) async fn increment_deliver_success(&self, address: &str) {
    let stats = self.stats_entry(address);
    stats.deliver_success.fetch_add(1, Ordering::SeqCst);
    if let Some(sink) = self.metrics_sink().await {
      let labels = vec![
        KeyValue::new("remote.endpoint", address.to_string()),
        KeyValue::new("remote.direction", "outbound"),
        KeyValue::new("remote.result", "success"),
      ];
      sink.increment_remote_delivery_success_with_labels(&labels);
    }
  }

  pub(crate) async fn increment_deliver_failure(&self, address: &str) {
    let stats = self.stats_entry(address);
    stats.deliver_failure.fetch_add(1, Ordering::SeqCst);
    if let Some(sink) = self.metrics_sink().await {
      let labels = vec![
        KeyValue::new("remote.endpoint", address.to_string()),
        KeyValue::new("remote.direction", "outbound"),
        KeyValue::new("remote.result", "failure"),
      ];
      sink.increment_remote_delivery_failure_with_labels(&labels);
    }
  }

  pub(crate) fn record_reconnect_attempt_metric(&self, address: &str) -> u64 {
    let stats = self.stats_entry(address);
    stats.reconnect_attempts.fetch_add(1, Ordering::SeqCst) + 1
  }

  pub(crate) fn remove_statistics(&self, address: &str) {
    self.endpoint_stats.remove(address);
  }

  pub fn statistics_snapshot(&self, address: &str) -> Option<EndpointStatisticsSnapshot> {
    self.endpoint_stats.get(address).map(|entry| entry.value().snapshot())
  }

  fn stop_heartbeat_task(&self, address: &str) {
    if let Some((_, handle)) = self.heartbeat_tasks.remove(address) {
      handle.cancel();
    }
  }

  fn start_heartbeat_monitor(&self, address: String, state: EndpointStateHandle) {
    let interval_duration = state.heartbeat_config().interval();
    let timeout = state.heartbeat_config().timeout();
    let manager = self.clone();
    let tasks = self.heartbeat_tasks.clone();
    let address_for_task = address.clone();
    let Some(remote) = self.remote.upgrade() else {
      tracing::warn!("Remote dropped before starting heartbeat monitor");
      return;
    };
    let spawner = remote.get_actor_system().core_runtime().spawner();
    let future: CoreTaskFuture = Box::pin(async move {
      let mut ticker = interval(interval_duration);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        if manager.stopped.load(Ordering::SeqCst) {
          break;
        }

        match state.connection_state() {
          ConnectionState::Closed => break,
          ConnectionState::Suspended | ConnectionState::Reconnecting => continue,
          ConnectionState::Connected => {
            let elapsed = state.elapsed_since_last_heartbeat(Instant::now());
            if elapsed >= timeout {
              tracing::warn!(address = %address_for_task, ?elapsed, "Heartbeat timeout detected; scheduling reconnect");
              let event = EndpointEvent::EndpointTerminated(EndpointTerminatedEvent {
                address: address_for_task.clone(),
              });
              manager.endpoint_event(event).await;
              state.set_connection_state(ConnectionState::Suspended);
              manager.schedule_reconnect(address_for_task.clone()).await;
              break;
            }
          }
        }
      }

      let _ = tasks.remove(&address_for_task);
    });

    let handle = match spawner.spawn(future) {
      Ok(handle) => handle,
      Err(err) => {
        tracing::error!(error = ?err, "Failed to spawn heartbeat monitor task");
        return;
      }
    };

    if let Some(previous) = self.heartbeat_tasks.insert(address, handle) {
      previous.cancel();
    }
  }

  pub(crate) async fn mark_heartbeat(&self, address: &str) {
    let state = self.endpoint_state(address).await;
    state.mark_heartbeat(Instant::now());
  }

  async fn try_reconnect_once(&self, address: String, state: EndpointStateHandle) -> Result<(), String> {
    if self.stopped.load(Ordering::SeqCst) {
      return Ok(());
    }

    if self.endpoint_supervisor.lock().await.is_none() {
      tracing::warn!(address = %address, "Endpoint supervisor is not ready; cannot reconnect yet");
      return Err("endpoint supervisor not initialized".to_string());
    }

    self.connections.remove(&address);
    let lazy = EndpointLazy::new(self.clone(), &address);
    lazy.get().await.map_err(|err| err.to_string())?;
    self.connections.insert(address.clone(), lazy);

    state.set_connection_state(ConnectionState::Connected);
    state.reset_retries();
    state.mark_heartbeat(Instant::now());

    self
      .endpoint_event(EndpointEvent::EndpointConnected(EndpointConnectedEvent {
        address: address.clone(),
      }))
      .await;
    Ok(())
  }

  async fn endpoint_state(&self, address: &str) -> EndpointStateHandle {
    if let Some(state) = self.endpoint_states.get(address) {
      return state.value().clone();
    }

    let remote = self.get_remote().await;
    let reconnect_policy = ReconnectPolicy::new(
      remote.get_config().get_endpoint_reconnect_max_retries().await,
      remote.get_config().get_endpoint_reconnect_initial_backoff().await,
      remote.get_config().get_endpoint_reconnect_max_backoff().await,
    );
    let heartbeat = HeartbeatConfig::new(
      remote.get_config().get_endpoint_heartbeat_interval().await,
      remote.get_config().get_endpoint_heartbeat_timeout().await,
    );

    let state = Arc::new(EndpointState::new(address.to_string(), reconnect_policy, heartbeat));

    match self.endpoint_states.entry(address.to_string()) {
      Entry::Occupied(entry) => entry.get().clone(),
      Entry::Vacant(entry) => {
        entry.insert(state.clone());
        state
      }
    }
  }

  pub(crate) async fn update_connection_state(&self, address: &str, new_state: ConnectionState) {
    let state = self.endpoint_state(address).await;
    state.set_connection_state(new_state);
    if matches!(new_state, ConnectionState::Connected) {
      state.reset_retries();
    }
  }

  pub(crate) fn remove_endpoint_state(&self, address: &str) {
    if let Some((_, state)) = self.endpoint_states.remove(address) {
      state.set_connection_state(ConnectionState::Closed);
    }
  }

  pub(crate) async fn schedule_reconnect(&self, address: String) {
    if self.stopped.load(Ordering::SeqCst) {
      tracing::debug!(address = %address, "Skipping reconnect scheduling because manager is stopped");
      return;
    }

    let state = self.endpoint_state(&address).await;
    match state.connection_state() {
      ConnectionState::Closed => {
        tracing::debug!(address = %address, "Reconnect skipped because endpoint is already closed");
        return;
      }
      ConnectionState::Reconnecting => {
        tracing::debug!(address = %address, "Reconnect already in progress; ignoring duplicate request");
        return;
      }
      _ => {}
    }

    state.set_connection_state(ConnectionState::Reconnecting);
    let max_retries = state.reconnect_policy().max_retries();
    let initial_attempt = state.record_retry_attempt();
    self.record_reconnect_attempt_metric(&address);
    self
      .publish_reconnect_event(&address, initial_attempt as u64, false)
      .await;
    self.stop_heartbeat_task(&address);
    let Some(remote) = self.remote.upgrade() else {
      tracing::warn!(address = %address, "Remote dropped before scheduling reconnect");
      return;
    };
    let spawner = remote.get_actor_system().core_runtime().spawner();
    let manager = self.clone();
    let state_handle = state.clone();
    let address_for_task = address.clone();

    let future: CoreTaskFuture = Box::pin(async move {
      let mut attempt = initial_attempt;
      loop {
        state_handle.set_connection_state(ConnectionState::Reconnecting);

        if max_retries > 0 && attempt > max_retries {
          tracing::warn!(
            address = %address_for_task,
            retries = attempt - 1,
            "Reached maximum reconnect attempts; closing endpoint"
          );
          state_handle.set_connection_state(ConnectionState::Closed);
          manager
            .publish_reconnect_event(&address_for_task, attempt as u64, false)
            .await;
          manager
            .endpoint_event(EndpointEvent::EndpointTerminated(EndpointTerminatedEvent {
              address: address_for_task.clone(),
            }))
            .await;
          manager.remove_endpoint_state(&address_for_task);
          break;
        }

        let delay = state_handle.compute_backoff_delay(attempt);
        tracing::debug!(address = %address_for_task, attempt, ?delay, "Scheduling reconnect attempt");
        if !delay.is_zero() {
          sleep(delay).await;
        }

        if manager.stopped.load(Ordering::SeqCst) {
          break;
        }

        match manager
          .try_reconnect_once(address_for_task.clone(), state_handle.clone())
          .await
        {
          Ok(()) => {
            tracing::debug!(address = %address_for_task, "Reconnect attempt succeeded");
            manager
              .publish_reconnect_event(&address_for_task, attempt as u64, true)
              .await;
            manager.start_heartbeat_monitor(address_for_task.clone(), state_handle.clone());
            break;
          }
          Err(err) => {
            tracing::warn!(
              address = %address_for_task,
              attempt,
              error = %err,
              "Reconnect attempt failed; retrying"
            );
            state_handle.set_connection_state(ConnectionState::Suspended);
            attempt = state_handle.record_retry_attempt();
            let _metric_attempt = manager.record_reconnect_attempt_metric(&address_for_task);
            manager
              .publish_reconnect_event(&address_for_task, attempt as u64, false)
              .await;
            continue;
          }
        }
      }
    });

    match spawner.spawn(future) {
      Ok(handle) => {
        handle.detach();
      }
      Err(err) => {
        tracing::error!(error = ?err, address = %address, "Failed to spawn reconnect task");
        state.set_connection_state(ConnectionState::Closed);
      }
    }
  }

  pub async fn await_reconnect(&self, address: &str) -> ConnectionState {
    let state = self.endpoint_state(address).await;
    state.wait_until_connected_or_closed().await
  }

  async fn determine_backpressure_level(&self, _address: &str, capacity: usize, len: usize) -> BackpressureLevel {
    if capacity == 0 {
      return BackpressureLevel::Normal;
    }

    let ratio = len as f64 / capacity as f64;
    let remote = self.get_remote().await;
    let warning = remote.get_config().get_backpressure_warning_threshold().await;
    let critical = remote.get_config().get_backpressure_critical_threshold().await;

    let (warning, critical) = if warning >= critical {
      (critical.min(0.8), critical.max(0.9))
    } else {
      (warning, critical)
    };

    if ratio >= critical {
      BackpressureLevel::Critical
    } else if ratio >= warning {
      BackpressureLevel::Warning
    } else {
      BackpressureLevel::Normal
    }
  }

  async fn publish_backpressure_event(&self, address: &str, level: BackpressureLevel) {
    let actor_system = match self.remote.upgrade() {
      Some(remote) => remote.get_actor_system().clone(),
      None => return,
    };
    let event = EndpointThrottledEvent {
      address: address.to_string(),
      level: level as i32,
    };

    actor_system
      .get_event_stream()
      .await
      .publish(MessageHandle::new(event))
      .await;
  }

  async fn publish_reconnect_event(&self, address: &str, attempt: u64, success: bool) {
    let actor_system = match self.remote.upgrade() {
      Some(remote) => remote.get_actor_system().clone(),
      None => return,
    };
    let event = EndpointReconnectEvent {
      address: address.to_string(),
      attempt,
      success,
    };
    actor_system
      .get_event_stream()
      .await
      .publish(MessageHandle::new(event))
      .await;
  }

  pub async fn get_endpoint_supervisor(&self) -> Pid {
    let mg = self.endpoint_supervisor.lock().await;
    mg.clone().expect("Endpoint supervisor not set")
  }

  async fn set_endpoint_supervisor(&self, pid: Pid) {
    let mut mg = self.endpoint_supervisor.lock().await;
    *mg = Some(pid);
  }

  #[cfg_attr(not(test), allow(dead_code))]
  async fn reset_endpoint_supervisor(&self) {
    let mut mg = self.endpoint_supervisor.lock().await;
    *mg = None;
  }

  #[cfg_attr(not(test), allow(dead_code))]
  async fn get_endpoint_subscription(&self) -> Subscription {
    let subscription = self.endpoint_subscription.lock().await;
    subscription.clone().expect("Endpoint subscription not set")
  }

  async fn set_endpoint_subscription(&self, subscription: Subscription) {
    let mut sub = self.endpoint_subscription.lock().await;
    *sub = Some(subscription);
  }

  async fn reset_endpoint_subscription(&self) {
    let mut sub = self.endpoint_subscription.lock().await;
    *sub = None;
  }

  async fn get_activator_pid(&self) -> Pid {
    let pid = self.activator_pid.lock().await;
    pid.clone().expect("Activator PID not set")
  }

  async fn set_activator_pid(&self, pid: Pid) {
    let mut ap = self.activator_pid.lock().await;
    *ap = Some(pid);
  }

  async fn get_remote(&self) -> Arc<Remote> {
    self.remote.upgrade().expect("Remote has been dropped")
  }

  pub async fn start(&mut self) -> Result<(), EndpointManagerError> {
    let cloned_self = self.clone();
    let event_stream = self.get_actor_system().await.get_event_stream().await.clone();

    self
      .set_endpoint_subscription(
        event_stream
          .subscribe_with_predicate(
            EventHandler::new(move |msg| {
              let cloned_self = cloned_self.clone();
              async move {
                let msg = msg.to_typed::<EndpointEvent>().unwrap().clone();
                cloned_self.endpoint_event(msg).await;
              }
            }),
            Predicate::new(move |msg| msg.is_typed::<EndpointEvent>()),
          )
          .await,
      )
      .await;

    self.start_activator().await?;
    self.start_supervisor().await?;
    self.waiting(Duration::from_secs(3)).await?;

    tracing::info!("Started EndpointManager");
    Ok(())
  }

  async fn waiting(&self, duration: Duration) -> Result<(), EndpointManagerError> {
    let actor_system = self.get_actor_system().await;
    let root = actor_system.get_root_context().await;
    let pid = ExtendedPid::new(self.get_activator_pid().await);
    let f = root.request_future(pid, MessageHandle::new(Ping), duration).await;
    let msg = f
      .result()
      .await
      .map_err(|e| EndpointManagerError::Waiting(e.to_string()))?;
    if msg.is_typed::<Pong>() {
      Ok(())
    } else {
      Err(EndpointManagerError::Waiting("type mismatch".to_string()))
    }
  }

  pub async fn stop(&mut self) -> Result<(), EndpointManagerError> {
    self.stopped.store(true, Ordering::SeqCst);
    if let Err(err) = self.stop_activator().await {
      tracing::error!("Failed to stop activator: {:?}", err);
    }
    if let Err(err) = self.stop_supervisor().await {
      tracing::error!("Failed to stop supervisor: {:?}", err);
    }
    self.reset_endpoint_subscription().await;
    self.connections = Arc::new(DashMap::new());
    self.client_connections.clear();
    self.client_connection_keys.clear();
    let remaining_states: Vec<_> = self.endpoint_states.iter().map(|entry| entry.value().clone()).collect();
    for state in remaining_states {
      state.set_connection_state(ConnectionState::Closed);
    }
    self.endpoint_states.clear();
    for entry in self.heartbeat_tasks.iter() {
      entry.value().cancel();
    }
    self.heartbeat_tasks.clear();
    for value_ref in self.endpoint_reader_connections.iter() {
      let (key, value) = value_ref.pair();
      let sender = {
        let mg = value.lock().await;
        mg.as_ref().unwrap().clone()
      };
      if let Err(err) = sender.send(true).await {
        tracing::error!("Failed to send stop signal to endpoint reader: {:?}", err);
      }
      self.endpoint_reader_connections.remove(key);
    }
    tracing::info!("Stopped EndpointManager");
    Ok(())
  }

  async fn start_activator(&mut self) -> Result<(), EndpointManagerError> {
    let cloned_remote = self.remote.clone();
    let props = Props::from_async_actor_producer(move |_| {
      let cloned_remote = cloned_remote.clone();
      async move { Activator::new(cloned_remote.clone()) }
    })
    .await;
    let actor_system = self.get_actor_system().await;
    let mut root = actor_system.get_root_context().await;
    match root.spawn_named(props, "activator").await {
      Ok(pid) => {
        self.set_activator_pid(pid.inner_pid.clone()).await;
        Ok(())
      }
      Err(e) => {
        tracing::error!("Failed to start activator: {:?}", e);
        Err(EndpointManagerError::ActivatorStarted(e))
      }
    }
  }

  async fn stop_activator(&mut self) -> Result<(), EndpointManagerError> {
    let pid = ExtendedPid::new(self.get_activator_pid().await);
    let f = self
      .get_actor_system()
      .await
      .get_root_context()
      .await
      .stop_future(&pid)
      .await;
    match f.result().await {
      Ok(_) => Ok(()),
      Err(e) => Err(EndpointManagerError::ActivatorStopped(e)),
    }
  }

  async fn start_supervisor(&mut self) -> Result<(), EndpointManagerError> {
    tracing::debug!("Starting supervisor");
    let remote = self.remote.clone();
    let dispatcher = DispatcherHandle::new(
      SingleWorkerDispatcher::new()
        .expect("Failed to create dispatcher for EndpointSupervisor")
        .with_throughput(300),
    );
    let props = Props::from_async_actor_producer_with_opts(
      move |_| {
        let remote = remote.clone();
        async move { EndpointSupervisor::new(remote) }
      },
      [
        Props::with_guardian(SupervisorStrategyHandle::new(RestartingStrategy::new())),
        Props::with_supervisor_strategy(SupervisorStrategyHandle::new(RestartingStrategy::new())),
        Props::with_dispatcher(dispatcher.clone()),
      ],
    )
    .await;
    match self
      .get_actor_system()
      .await
      .get_root_context()
      .await
      .spawn_named(props, "EndpointSupervisor")
      .await
    {
      Ok(pid) => {
        tracing::debug!("Supervisor started");
        self.set_endpoint_supervisor(pid.inner_pid.clone()).await;
        Ok(())
      }
      Err(e) => {
        tracing::error!("Failed to start supervisor: {:?}", e);
        Err(EndpointManagerError::SupervisorStarted(e))
      }
    }
  }

  async fn stop_supervisor(&mut self) -> Result<(), EndpointManagerError> {
    let pid = ExtendedPid::new(self.get_endpoint_supervisor().await);
    let f = self
      .get_actor_system()
      .await
      .get_root_context()
      .await
      .stop_future(&pid)
      .await;
    f.result()
      .await
      .map(|_| ())
      .map_err(EndpointManagerError::SupervisorStopped)
  }

  pub async fn endpoint_event(&self, endpoint_event: EndpointEvent) {
    match &endpoint_event {
      EndpointEvent::EndpointTerminated(ev) => {
        tracing::debug!("EndpointManager received endpoint terminated event, removing endpoint");
        self.remove_endpoint(ev).await;
      }
      EndpointEvent::EndpointConnected(ev) => {
        let endpoint = self.ensure_connected(&ev.address).await;
        let pid = ExtendedPid::new(endpoint.get_watcher().clone());
        self
          .get_actor_system()
          .await
          .get_root_context()
          .await
          .send(pid, MessageHandle::new(endpoint_event))
          .await;
      }
    }
  }

  pub async fn remote_terminate(&self, message: &RemoteTerminate) {
    if self.stopped.load(Ordering::SeqCst) {
      return;
    }
    let address = message.watchee.as_ref().expect("Not Found").address.clone();
    if let Some(watcher) = message.watcher.as_ref() {
      if let Some(registry) = self.watch_registry(&address) {
        if let Some(stat) = registry.remove_watchee(&watcher.id, message.watchee.as_ref()).await {
          if stat.changed {
            self
              .publish_watch_event(
                &address,
                &watcher.id,
                message.watchee.clone(),
                WatchAction::Terminate,
                stat.watchers,
              )
              .await;
          }
        }
      }
    }
    let endpoint = self.ensure_connected(&address).await;
    let pid = ExtendedPid::new(endpoint.get_watcher().clone());
    self
      .get_actor_system()
      .await
      .get_root_context()
      .await
      .send(pid, MessageHandle::new(message.clone()))
      .await;
  }

  pub(crate) async fn remote_watch(&self, message: RemoteWatch) {
    if self.stopped.load(Ordering::SeqCst) {
      return;
    }
    let address = message.watchee.address.clone();
    let mut should_forward = true;
    if let Some(registry) = self.watch_registry(&address) {
      let stat = registry.watch(&message.watcher.id, message.watchee.clone()).await;
      if stat.changed {
        self
          .publish_watch_event(
            &address,
            &message.watcher.id,
            Some(message.watchee.clone()),
            WatchAction::Watch,
            stat.watchers,
          )
          .await;
      } else {
        should_forward = false;
      }
    }
    if should_forward {
      let endpoint = self.ensure_connected(&address).await;
      let pid = ExtendedPid::new(endpoint.get_watcher().clone());
      self
        .get_actor_system()
        .await
        .get_root_context()
        .await
        .send(pid, MessageHandle::new(message))
        .await;
    }
  }

  pub(crate) async fn remote_unwatch(&self, message: RemoteUnwatch) {
    if self.stopped.load(Ordering::SeqCst) {
      return;
    }
    let address = message.watchee.address.clone();
    let mut should_forward = true;
    if let Some(registry) = self.watch_registry(&address) {
      if let Some(stat) = registry.unwatch(&message.watcher.id, &message.watchee).await {
        if stat.changed {
          self
            .publish_watch_event(
              &address,
              &message.watcher.id,
              Some(message.watchee.clone()),
              WatchAction::Unwatch,
              stat.watchers,
            )
            .await;
        } else {
          should_forward = false;
        }
      } else {
        should_forward = false;
      }
    }
    if should_forward {
      let endpoint = self.ensure_connected(&address).await;
      let pid = ExtendedPid::new(endpoint.get_watcher().clone());
      self
        .get_actor_system()
        .await
        .get_root_context()
        .await
        .send(pid, MessageHandle::new(message))
        .await;
    }
  }

  pub(crate) async fn remote_deliver(&self, message: RemoteDeliver) {
    if self.stopped.load(Ordering::SeqCst) {
      let pid = ExtendedPid::new(message.target.clone());
      let sender = message.sender.map(ExtendedPid::new);
      self
        .get_actor_system()
        .await
        .get_event_stream()
        .await
        .publish(MessageHandle::new(DeadLetterEvent {
          pid: Some(pid),
          message_handle: message.message.clone(),
          sender,
        }))
        .await;
      return;
    }
    let address = message.target.address.clone();
    let endpoint = self.ensure_connected(&address).await;
    let pid = ExtendedPid::new(endpoint.get_writer().clone());
    self
      .get_actor_system()
      .await
      .get_root_context()
      .await
      .send(pid, MessageHandle::new(message))
      .await;
  }

  async fn ensure_connected(&self, address: &str) -> Endpoint {
    let (endpoint_lazy, newly_created) = if let Some(existing) = self.connections.get(address) {
      (existing.value().clone(), false)
    } else {
      let lazy = EndpointLazy::new(self.clone(), address);
      let (value, already_present) = self.connections.load_or_store(address.to_string(), lazy.clone());
      (value, !already_present)
    };

    let endpoint = endpoint_lazy.get().await.expect("Endpoint is not found").clone();

    let state = self.endpoint_state(address).await;
    self.update_connection_state(address, ConnectionState::Connected).await;

    let should_start_monitor = newly_created || !self.heartbeat_tasks.contains_key(address);
    state.mark_heartbeat(Instant::now());

    if should_start_monitor {
      self.start_heartbeat_monitor(address.to_string(), state.clone());
      self.record_queue_state(address, 0, 0).await;
    }

    endpoint
  }

  async fn publish_watch_event(
    &self,
    address: &str,
    watcher: &str,
    watchee: Option<Pid>,
    action: WatchAction,
    watchers: usize,
  ) {
    let event_stream = self.get_actor_system().await.get_event_stream().await;
    let watchers_u32 = u32::try_from(watchers).unwrap_or(u32::MAX);
    event_stream
      .publish(MessageHandle::new(EndpointWatchEvent {
        address: address.to_string(),
        watcher: watcher.to_string(),
        watchee: watchee.clone(),
        action,
        watchers: watchers_u32,
      }))
      .await;

    if let Some(sink) = self.metrics_sink().await {
      let event_labels = [
        KeyValue::new("remote.endpoint", address.to_string()),
        KeyValue::new("remote.watcher", watcher.to_string()),
        KeyValue::new("remote.watch_action", action.as_str().to_string()),
      ];
      let gauge_labels = [
        KeyValue::new("remote.endpoint", address.to_string()),
        KeyValue::new("remote.watcher", watcher.to_string()),
      ];
      sink.increment_remote_watch_event_with_labels(&event_labels);
      sink.record_remote_watchers_with_labels(watchers_u32, &gauge_labels);
    }
  }

  async fn remove_endpoint(&self, message: &EndpointTerminatedEvent) {
    self.stop_heartbeat_task(&message.address);
    self.watch_registries.remove(&message.address);
    if let Some(v) = self.connections.get(&message.address) {
      let le = v.value();
      if le
        .get_unloaded()
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
      {
        self.connections.remove(&message.address);
        self.remove_statistics(&message.address);
        let ep = le.get().await.expect("Endpoint not found");
        tracing::debug!(
          "Sending EndpointTerminatedEvent to EndpointWatcher and EndpointWriter, address: {}",
          message.address
        );
        let watcher_pid = ExtendedPid::new(ep.get_watcher().clone());
        let writer_pid = ExtendedPid::new(ep.get_writer().clone());
        let msg = EndpointEvent::EndpointTerminated(message.clone());
        self
          .get_actor_system()
          .await
          .get_root_context()
          .await
          .send(watcher_pid, MessageHandle::new(msg.clone()))
          .await;
        self
          .get_actor_system()
          .await
          .get_root_context()
          .await
          .send(writer_pid, MessageHandle::new(msg))
          .await;
      }
    }
    self.remove_endpoint_state(&message.address);
  }

  pub(crate) async fn get_actor_system(&self) -> ActorSystem {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_actor_system()
      .clone()
  }

  #[allow(clippy::type_complexity)]
  pub(crate) fn get_endpoint_reader_connections(
    &self,
  ) -> Arc<DashMap<RequestKeyWrapper, Arc<Mutex<Option<mpsc::Sender<bool>>>>>> {
    self.endpoint_reader_connections.clone()
  }

  pub(crate) fn register_client_connection(
    &self,
    system_id: String,
    key: RequestKeyWrapper,
    sender: ClientResponseSender,
  ) {
    if let Some(old_system_id) = self.client_connection_keys.insert(key.clone(), system_id.clone()) {
      self.client_connections.remove(&old_system_id);
    }
    self.client_connections.insert(system_id, sender);
  }

  pub(crate) fn deregister_client_connection(&self, key: &RequestKeyWrapper) {
    if let Some((_, system_id)) = self.client_connection_keys.remove(key) {
      self.client_connections.remove(&system_id);
    }
  }

  #[cfg(test)]
  pub(crate) fn has_client_connection(&self, system_id: &str) -> bool {
    self.client_connections.contains_key(system_id)
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) async fn send_to_client(&self, system_id: &str, message: RemoteMessage) -> Result<(), ClientSendError> {
    let sender = self
      .client_connections
      .get(system_id)
      .ok_or_else(|| ClientSendError::NotFound(system_id.to_string()))?
      .clone();
    sender
      .send(Ok(message))
      .await
      .map_err(|err| ClientSendError::SendFailed(system_id.to_string(), err.to_string()))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Config;
  use crate::config_option::ConfigOption;
  use crate::endpoint::Endpoint;
  use crate::endpoint_watcher::EndpointWatcher;
  use crate::messages::EndpointReconnectEvent;
  use crate::remote::Remote;
  use crate::watch_registry::WatchRegistry;
  use async_trait::async_trait;
  use nexus_actor_std_rs::actor::actor_system::ActorSystem;
  use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart};
  use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
  use nexus_actor_std_rs::actor::message::{MessageHandle, ResponseHandle};
  use nexus_actor_std_rs::generated::actor::Pid;
  use std::sync::Arc;
  use std::time::Duration;

  #[derive(Debug, Clone)]
  struct NoopActor;

  #[async_trait]
  impl Actor for NoopActor {
    async fn receive(&mut self, _context_handle: ContextHandle) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[derive(Debug, Clone)]
  struct SupervisorStub {
    endpoint: Endpoint,
  }

  #[async_trait]
  impl Actor for SupervisorStub {
    async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
      if context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
        .to_typed::<String>()
        .is_some()
      {
        context_handle.respond(ResponseHandle::new(self.endpoint.clone())).await;
      }
      Ok(())
    }
  }

  #[tokio::test]
  async fn endpoint_manager_internal_setters_are_exercised() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system, Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));

    let pid = Pid {
      address: "remote-system".into(),
      id: "pid-1".into(),
      request_id: 0,
    };

    manager.set_endpoint_supervisor(pid.clone()).await;
    assert_eq!(manager.get_endpoint_supervisor().await, pid);
    manager.reset_endpoint_supervisor().await;
    assert!(manager.endpoint_supervisor.lock().await.is_none());

    manager.set_endpoint_supervisor(pid.clone()).await;
    manager.set_activator_pid(pid.clone()).await;
    assert_eq!(manager.get_activator_pid().await, pid);

    let handler = Arc::new(EventHandler::new(|_| async {}));
    let subscription = Subscription::new(1, handler.clone(), None);
    manager.set_endpoint_subscription(subscription.clone()).await;
    assert_eq!(manager.get_endpoint_subscription().await, subscription);
    manager.reset_endpoint_subscription().await;
    assert!(manager.endpoint_subscription.lock().await.is_none());

    let upgraded = manager.get_remote().await;
    assert!(Arc::ptr_eq(&upgraded, &remote));
  }

  #[tokio::test]
  async fn endpoint_manager_statistics_track_updates() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system, Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));

    manager.record_queue_state("endpoint-A", 10, 3).await;
    manager.increment_dead_letter("endpoint-A").await;

    let snapshot = manager
      .statistics_snapshot("endpoint-A")
      .expect("statistics should exist");
    assert_eq!(snapshot.queue_capacity, 10);
    assert_eq!(snapshot.queue_size, 3);
    assert_eq!(snapshot.dead_letters, 1);
    assert_eq!(snapshot.backpressure_level, BackpressureLevel::Normal);
    assert_eq!(snapshot.deliver_success, 0);
    assert_eq!(snapshot.deliver_failure, 0);
    assert_eq!(snapshot.reconnect_attempts, 0);

    manager.remove_statistics("endpoint-A");
    assert!(manager.statistics_snapshot("endpoint-A").is_none());
  }

  #[tokio::test]
  async fn endpoint_manager_delivery_metrics_increment() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system, Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));

    manager.increment_deliver_success("endpoint-metrics").await;
    manager.increment_deliver_failure("endpoint-metrics").await;
    manager.increment_deliver_failure("endpoint-metrics").await;

    let snapshot = manager
      .statistics_snapshot("endpoint-metrics")
      .expect("statistics should exist");
    assert_eq!(snapshot.deliver_success, 1);
    assert_eq!(snapshot.deliver_failure, 2);
  }

  #[tokio::test]
  async fn endpoint_state_lifecycle() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system, Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));

    let state = manager.endpoint_state("endpoint-B").await;
    assert_eq!(state.address(), "endpoint-B");
    assert_eq!(state.connection_state(), ConnectionState::Suspended);
    assert_eq!(state.reconnect_policy().max_retries(), 5);
    assert_eq!(state.reconnect_policy().initial_backoff(), Duration::from_millis(200));
    assert_eq!(state.reconnect_policy().max_backoff(), Duration::from_secs(5));

    manager
      .update_connection_state("endpoint-B", ConnectionState::Connected)
      .await;
    assert_eq!(state.connection_state(), ConnectionState::Connected);
    assert_eq!(state.retries(), 0);

    manager.schedule_reconnect("endpoint-B".to_string()).await;
    assert_eq!(state.connection_state(), ConnectionState::Reconnecting);
    assert_eq!(state.retries(), 1);

    manager.remove_endpoint_state("endpoint-B");
    assert!(manager.endpoint_states.get("endpoint-B").is_none());
  }

  #[tokio::test]
  async fn endpoint_backpressure_event_emitted() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system.clone(), Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));

    let events = Arc::new(Mutex::new(Vec::new()));
    let subscription = actor_system
      .get_event_stream()
      .await
      .subscribe({
        let events = events.clone();
        move |message: MessageHandle| {
          let events = events.clone();
          async move {
            if let Some(event) = message.to_typed::<EndpointThrottledEvent>() {
              events.lock().await.push(event);
            }
          }
        }
      })
      .await;

    manager.record_queue_state("endpoint-C", 10, 9).await;

    tokio::time::sleep(Duration::from_millis(10)).await;

    let events_guard = events.lock().await;
    assert_eq!(events_guard.len(), 1);
    assert_eq!(events_guard[0].address, "endpoint-C");
    assert_eq!(events_guard[0].level, BackpressureLevel::Critical as i32);
    drop(events_guard);

    actor_system.get_event_stream().await.unsubscribe(subscription).await;

    let snapshot = manager
      .statistics_snapshot("endpoint-C")
      .expect("snapshot should exist");
    assert_eq!(snapshot.backpressure_level, BackpressureLevel::Critical);
  }

  #[tokio::test]
  async fn remote_watch_updates_shared_registry() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system.clone(), Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));
    remote.set_endpoint_manager_for_test(manager.clone()).await;

    let registry = Arc::new(WatchRegistry::new());
    let address = "endpoint-watch-registry".to_string();
    let remote_weak = Arc::downgrade(&remote);

    let watch_events = Arc::new(Mutex::new(Vec::new()));
    let watch_subscription = actor_system
      .get_event_stream()
      .await
      .subscribe({
        let events = watch_events.clone();
        move |message: MessageHandle| {
          let events = events.clone();
          async move {
            if let Some(event) = message.to_typed::<EndpointWatchEvent>() {
              events.lock().await.push(event);
            }
          }
        }
      })
      .await;

    let watcher_props = Props::from_async_actor_producer({
      let registry = registry.clone();
      let address = address.clone();
      move |_| {
        let registry = registry.clone();
        let address = address.clone();
        let remote = remote_weak.clone();
        async move { EndpointWatcher::with_registry(remote, address, registry) }
      }
    })
    .await;
    let noop_props = Props::from_async_actor_producer(|_| async { NoopActor }).await;

    let watcher_pid = actor_system.get_root_context().await.spawn(watcher_props).await;
    let writer_pid = actor_system.get_root_context().await.spawn(noop_props.clone()).await;

    let endpoint = Endpoint::new(writer_pid.inner_pid.clone(), watcher_pid.inner_pid.clone());
    let supervisor_props = Props::from_async_actor_producer({
      let endpoint = endpoint.clone();
      move |_| {
        let endpoint = endpoint.clone();
        async move {
          SupervisorStub {
            endpoint: endpoint.clone(),
          }
        }
      }
    })
    .await;
    let supervisor_pid: ExtendedPid = actor_system.get_root_context().await.spawn(supervisor_props).await;

    manager.set_endpoint_supervisor(supervisor_pid.inner_pid.clone()).await;
    manager.register_watch_registry(&address, registry.clone());

    let watcher_pid_struct = Pid {
      address: "local-system".into(),
      id: "watcher-pid".into(),
      request_id: 0,
    };
    let watchee_pid = Pid {
      address: address.clone(),
      id: "watchee-pid".into(),
      request_id: 1,
    };

    manager
      .remote_watch(RemoteWatch {
        watcher: watcher_pid_struct.clone(),
        watchee: watchee_pid.clone(),
      })
      .await;

    let pid_set = registry
      .get_pid_set(&watcher_pid_struct.id)
      .expect("watch registry should exist");
    assert_eq!(pid_set.len().await, 1);
    assert!(pid_set.contains(&watchee_pid).await);

    manager
      .remote_watch(RemoteWatch {
        watcher: watcher_pid_struct.clone(),
        watchee: watchee_pid.clone(),
      })
      .await;
    assert_eq!(pid_set.len().await, 1);

    manager
      .remote_unwatch(RemoteUnwatch {
        watcher: watcher_pid_struct.clone(),
        watchee: watchee_pid.clone(),
      })
      .await;
    let remaining = registry.get_pid_set(&watcher_pid_struct.id);
    assert!(remaining.is_none() || remaining.unwrap().is_empty().await);

    manager
      .remote_watch(RemoteWatch {
        watcher: watcher_pid_struct.clone(),
        watchee: watchee_pid.clone(),
      })
      .await;
    manager
      .remote_terminate(&RemoteTerminate {
        watcher: Some(watcher_pid_struct.clone()),
        watchee: Some(watchee_pid.clone()),
      })
      .await;

    assert!(registry.get_pid_set(&watcher_pid_struct.id).is_none());

    tokio::time::sleep(Duration::from_millis(10)).await;

    let events = watch_events.lock().await;
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0].action, WatchAction::Watch));
    assert_eq!(events[0].watchers, 1);
    assert!(matches!(events[1].action, WatchAction::Unwatch));
    assert_eq!(events[1].watchers, 0);
    assert!(matches!(events[2].action, WatchAction::Watch));
    assert_eq!(events[2].watchers, 1);
    assert!(matches!(events[3].action, WatchAction::Terminate));
    assert_eq!(events[3].watchers, 0);
    drop(events);

    actor_system
      .get_event_stream()
      .await
      .unsubscribe(watch_subscription)
      .await;
  }

  #[tokio::test]
  async fn schedule_reconnect_emits_events_and_metrics() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let config = Config::from(vec![
      ConfigOption::with_endpoint_reconnect_max_retries(1),
      ConfigOption::with_endpoint_reconnect_initial_backoff(Duration::from_millis(0)),
      ConfigOption::with_endpoint_reconnect_max_backoff(Duration::from_millis(0)),
    ])
    .await;
    let remote = Arc::new(Remote::new(actor_system.clone(), config).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));

    let events = Arc::new(Mutex::new(Vec::new()));
    let subscription = actor_system
      .get_event_stream()
      .await
      .subscribe({
        let events = events.clone();
        move |message: MessageHandle| {
          let events = events.clone();
          async move {
            if let Some(event) = message.to_typed::<EndpointReconnectEvent>() {
              events.lock().await.push(event);
            }
          }
        }
      })
      .await;

    manager
      .schedule_reconnect("endpoint-reconnect-metrics".to_string())
      .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let snapshot = manager
      .statistics_snapshot("endpoint-reconnect-metrics")
      .expect("statistics should exist");
    assert!(snapshot.reconnect_attempts >= 1);

    let events_guard = events.lock().await;
    assert!(!events_guard.is_empty());
    actor_system.get_event_stream().await.unsubscribe(subscription).await;
  }

  #[tokio::test]
  async fn reconnect_gives_up_after_max_attempts() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let config = Config::from(vec![
      ConfigOption::with_endpoint_reconnect_max_retries(1),
      ConfigOption::with_endpoint_reconnect_initial_backoff(Duration::from_millis(0)),
      ConfigOption::with_endpoint_reconnect_max_backoff(Duration::from_millis(0)),
    ])
    .await;
    let remote = Arc::new(Remote::new(actor_system, config).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));
    remote.set_endpoint_manager_for_test(manager.clone()).await;

    let state = manager.endpoint_state("endpoint-reconnect").await;
    manager.schedule_reconnect("endpoint-reconnect".to_string()).await;

    tokio::time::sleep(Duration::from_millis(10)).await;

    assert_eq!(state.connection_state(), ConnectionState::Closed);
  }

  #[tokio::test]
  async fn await_reconnect_resolves_connected_state() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system, Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));
    remote.set_endpoint_manager_for_test(manager.clone()).await;

    let waiter = {
      let manager = manager.clone();
      async move { manager.await_reconnect("endpoint-await-connected").await }
    };

    let updater = async {
      tokio::time::sleep(Duration::from_millis(10)).await;
      manager
        .update_connection_state("endpoint-await-connected", ConnectionState::Connected)
        .await;
    };

    let (_, result) = tokio::join!(updater, waiter);
    assert_eq!(result, ConnectionState::Connected);
  }

  #[tokio::test]
  async fn await_reconnect_resolves_closed_state_on_removal() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let remote = Arc::new(Remote::new(actor_system, Config::default()).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));
    remote.set_endpoint_manager_for_test(manager.clone()).await;

    let waiter = {
      let manager = manager.clone();
      async move { manager.await_reconnect("endpoint-await-closed").await }
    };

    let remover = async {
      tokio::time::sleep(Duration::from_millis(10)).await;
      manager.remove_endpoint_state("endpoint-await-closed");
    };

    let (_, result) = tokio::join!(remover, waiter);
    assert_eq!(result, ConnectionState::Closed);
  }

  #[tokio::test]
  async fn schedule_reconnect_successfully_restores_connection() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let config = Config::from(vec![
      ConfigOption::with_endpoint_reconnect_initial_backoff(Duration::from_millis(0)),
      ConfigOption::with_endpoint_reconnect_max_backoff(Duration::from_millis(0)),
    ])
    .await;
    let remote = Arc::new(Remote::new(actor_system.clone(), config).await);
    let manager = EndpointManager::new(Arc::downgrade(&remote));
    remote.set_endpoint_manager_for_test(manager.clone()).await;

    let noop_props = Props::from_async_actor_producer(|_| async { NoopActor }).await;
    let writer_pid = actor_system.get_root_context().await.spawn(noop_props.clone()).await;
    let watcher_pid = actor_system.get_root_context().await.spawn(noop_props.clone()).await;

    let endpoint = Endpoint::new(writer_pid.inner_pid.clone(), watcher_pid.inner_pid.clone());
    let supervisor_props = Props::from_async_actor_producer(move |_| {
      let endpoint = endpoint.clone();
      async move {
        SupervisorStub {
          endpoint: endpoint.clone(),
        }
      }
    })
    .await;
    let supervisor_pid: ExtendedPid = actor_system.get_root_context().await.spawn(supervisor_props).await;

    manager.set_endpoint_supervisor(supervisor_pid.inner_pid.clone()).await;

    let address = "endpoint-reconnect-success";
    let wait_future = {
      let remote = remote.clone();
      let address = address.to_string();
      async move { remote.await_reconnect(&address).await }
    };

    let schedule_future = async {
      manager.schedule_reconnect(address.to_string()).await;
    };

    let state = tokio::time::timeout(Duration::from_secs(1), async {
      let (_, result) = tokio::join!(schedule_future, wait_future);
      result
    })
    .await
    .expect("await_reconnect timed out");

    assert_eq!(state, Some(ConnectionState::Connected));
  }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClientSendError {
  #[error("client connection not found: {0}")]
  NotFound(String),
  #[error("failed to send to client {0}: {1}")]
  SendFailed(String, String),
}
