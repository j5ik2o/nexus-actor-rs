use crate::actor::actor::{ActorProducer, ExtendedPid, Props, SpawnError};
use crate::actor::context::{SenderPart, SpawnerPart, StopperPart};
use crate::actor::dispatch::future::ActorFutureError;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::event_stream::{EventHandler, Predicate, Subscription};
use crate::generated::actor::Pid;
use crate::generated::remote::RemoteMessage;
use crate::remote::activator_actor::Activator;
use crate::remote::endpoint_lazy::EndpointLazy;
use crate::remote::messages::{EndpointEvent, Ping};
use crate::remote::remote::Remote;
use dashmap::DashMap;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

impl Eq for RemoteMessage {}

impl Hash for RemoteMessage {
  fn hash<H: Hasher>(&self, state: &mut H) {
    // self.message_type のハッシュ値に31を掛けたものを返す
  }
}

#[derive(Clone)]
pub(crate) struct EndpointManager {
  connections: Arc<Mutex<DashMap<String, EndpointLazy>>>,
  remote: Arc<Mutex<Remote>>,
  endpoint_subscription: Option<Subscription>,
  endpoint_supervisor: Option<Pid>,
  activator: Option<Pid>,
  stopped: bool,
  endpoint_reader_connections: Arc<Mutex<DashMap<RemoteMessage, tokio::sync::oneshot::Sender<bool>>>>,
}

impl EndpointManager {
  pub fn new(remote: Arc<Mutex<Remote>>) -> Self {
    EndpointManager {
      connections: Arc::new(Mutex::new(DashMap::new())),
      remote,
      endpoint_subscription: None,
      endpoint_supervisor: None,
      activator: None,
      stopped: false,
      endpoint_reader_connections: Arc::new(Mutex::new(DashMap::new())),
    }
  }

  pub fn endpoint_event(&self, endpoint_event: EndpointEvent) {
    match &endpoint_event {
      EndpointEvent::EndpointTerminated(_) => {
        // self.remove_endpoint(endpoint_event);
      }
      EndpointEvent::EndpointConnected(_) => {}
    }
  }

  pub async fn start(&mut self) -> Result<MessageHandle, ActorFutureError> {
    let cloned_self = self.clone();
    let event_stream = {
      let mg = cloned_self.remote.lock().await;
      mg.get_actor_system().get_event_stream().await
    };
    self.endpoint_subscription = Some(
      event_stream
        .subscribe_with_predicate(
          EventHandler::new(move |msg| {
            let cloned_self = cloned_self.clone();
            async move {
              let msg = msg.to_typed::<EndpointEvent>().unwrap().clone();
              cloned_self.endpoint_event(msg);
            }
          }),
          Predicate::new(move |msg| msg.is_typed::<EndpointEvent>()),
        )
        .await,
    );
    let _ = self.start_activator().await;
    let _ = self.start_supervisor().await;

    let result = self.waiting(Duration::from_secs(3)).await;

    tracing::info!("Started EndpointManager");

    result
  }

  async fn waiting(&self, duration: Duration) -> Result<MessageHandle, ActorFutureError> {
    let actor_system = {
      let mg = self.remote.lock().await;
      mg.get_actor_system().clone()
    };
    let root = actor_system.get_root_context().await;
    let pid = ExtendedPid::new(self.activator.clone().unwrap(), actor_system.clone());
    let f = root.request_future(pid, MessageHandle::new(Ping), duration).await;
    f.result().await
  }

  async fn start_activator(&mut self) -> Result<ExtendedPid, SpawnError> {
    let cloned_self = self.clone();
    let props = Props::from_actor_producer(move |ctx| {
      let cloned_self = cloned_self.remote.clone();
      async move { Activator::new(cloned_self.clone()) }
    })
    .await;
    let actor_system = {
      let mg = self.remote.lock().await;
      mg.get_actor_system().clone()
    };
    let mut root = actor_system.get_root_context().await;
    let result = root.spawn_named(props, "activator").await;
    match &result {
      Ok(pid) => {
        self.activator = Some(pid.inner_pid.clone());
      }
      Err(e) => {
        tracing::error!("Failed to start activator: {:?}", e);
      }
    }
    result
  }

  async fn start_supervisor(&mut self) -> Result<MessageHandle, ActorFutureError> {
    let actor_system = {
      let mg = self.remote.lock().await;
      mg.get_actor_system().clone()
    };
    let mut root = actor_system.get_root_context().await;
    let pid = ExtendedPid::new(self.activator.clone().unwrap(), actor_system.clone());
    let f = root.stop_future(&pid).await;
    f.result().await
  }
}

impl Debug for EndpointManager {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EndpointManager")
      .field("connections", &self.connections)
      .finish()
  }
}
