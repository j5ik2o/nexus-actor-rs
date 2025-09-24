use crate::activator_actor::Activator;
use crate::endpoint::Endpoint;
use crate::endpoint_lazy::EndpointLazy;
use crate::endpoint_supervisor::EndpointSupervisor;
use crate::generated::remote::RemoteMessage;
use crate::messages::{
  EndpointEvent, EndpointTerminatedEvent, Ping, Pong, RemoteDeliver, RemoteTerminate, RemoteUnwatch, RemoteWatch,
};
use crate::remote::Remote;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{SenderPart, SpawnerPart, StopperPart};
use nexus_actor_core_rs::actor::core::{ExtendedPid, Props, SpawnError};
use nexus_actor_core_rs::actor::dispatch::DeadLetterEvent;
use nexus_actor_core_rs::actor::message::MessageHandle;
use nexus_actor_core_rs::actor::process::future::ActorFutureError;
use nexus_actor_core_rs::actor::supervisor::{RestartingStrategy, SupervisorStrategyHandle};
use nexus_actor_core_rs::event_stream::{EventHandler, Predicate, Subscription};
use nexus_actor_core_rs::generated::actor::Pid;
use nexus_actor_utils_rs::collections::DashMapExtension;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
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

#[derive(Debug, Clone)]
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
    }
  }

  pub async fn get_endpoint_supervisor(&self) -> Pid {
    let mg = self.endpoint_supervisor.lock().await;
    mg.clone().expect("Endpoint supervisor not set")
  }

  async fn set_endpoint_supervisor(&self, pid: Pid) {
    let mut mg = self.endpoint_supervisor.lock().await;
    *mg = Some(pid);
  }

  async fn reset_endpoint_supervisor(&self) {
    let mut mg = self.endpoint_supervisor.lock().await;
    *mg = None;
  }

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
    let props = Props::from_async_actor_producer_with_opts(
      move |_| {
        let remote = remote.clone();
        async move { EndpointSupervisor::new(remote) }
      },
      [
        Props::with_guardian(SupervisorStrategyHandle::new(RestartingStrategy::new())),
        Props::with_supervisor_strategy(SupervisorStrategyHandle::new(RestartingStrategy::new())),
        // TODO:
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

  pub(crate) async fn remote_unwatch(&self, message: RemoteUnwatch) {
    if self.stopped.load(Ordering::SeqCst) {
      return;
    }
    let address = message.watchee.address.clone();
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
    match self.connections.get(address) {
      None => {
        let el = EndpointLazy::new(self.clone(), address);
        let (el2, _) = self.connections.load_or_store(address.to_string(), el);
        el2.get().await.expect("Endpoint is not found").clone()
      }
      Some(v) => v.get().await.expect("Endpoint is not found").clone(),
    }
  }

  async fn remove_endpoint(&self, message: &EndpointTerminatedEvent) {
    if let Some(v) = self.connections.get(&message.address) {
      let le = v.value();
      if le
        .get_unloaded()
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
      {
        self.connections.remove(&message.address);
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClientSendError {
  #[error("client connection not found: {0}")]
  NotFound(String),
  #[error("failed to send to client {0}: {1}")]
  SendFailed(String, String),
}
