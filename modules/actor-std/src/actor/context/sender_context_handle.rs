use async_trait::async_trait;
use std::time::Duration;
use tracing::{debug, warn};

use crate::actor::actor_system::{with_actor_system, ActorSystem};
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::context_registry::ContextRegistry;
use crate::actor::context::root_context::RootContext;
use crate::actor::context::{CoreSenderPart, InfoPart, MessagePart, SenderContext, SenderPart};
use crate::actor::core::ActorHandle;
use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::process::actor_future::ActorFuture;
use nexus_actor_core_rs::context::CoreSenderSnapshot;
use nexus_actor_core_rs::CorePid;

#[derive(Debug, Clone)]
enum SenderContextRef {
  Context(ContextHandle),
  Root(RootContext),
}

#[derive(Debug, Clone)]
pub struct SenderContextHandle(SenderContextRef);

impl SenderContextHandle {
  pub fn new(context: ContextHandle) -> Self {
    SenderContextHandle::from_context(context)
  }

  pub fn from_context(context: ContextHandle) -> Self {
    SenderContextHandle(SenderContextRef::Context(context))
  }

  pub fn from_root(root: RootContext) -> Self {
    SenderContextHandle(SenderContextRef::Root(root))
  }

  pub fn try_sender(&self) -> Option<ExtendedPid> {
    match &self.0 {
      SenderContextRef::Context(context) => context.try_get_sender_opt(),
      SenderContextRef::Root(_) => None,
    }
  }

  pub fn try_sender_core(&self) -> Option<CorePid> {
    self.try_sender().map(|pid| pid.to_core())
  }

  pub async fn capture_core_snapshot(&self) -> Option<CoreSenderSnapshot> {
    match &self.0 {
      SenderContextRef::Context(context) => {
        let actor_system = context.get_actor_system().await;
        let system_id = actor_system.system_id();
        let self_pid = context.get_self_opt().await?;
        ContextRegistry::register(system_id, self_pid.id(), context);
        let core_snapshot = context.core_snapshot().await;
        Some(CoreSenderSnapshot::new(core_snapshot, system_id))
      }
      SenderContextRef::Root(_) => None,
    }
  }

  pub async fn from_core_snapshot(snapshot: &CoreSenderSnapshot) -> Option<Self> {
    let (context_snapshot, system_id) = snapshot.clone().into_parts();
    let self_pid = context_snapshot.self_pid_core();
    if let Some(handle) = ContextRegistry::get(system_id, self_pid.id()) {
      debug!(
        target = "nexus::context_registry",
        system_id,
        pid = %self_pid.id(),
        "Sender context restored from registry entry"
      );
      return Some(SenderContextHandle::from_context(handle));
    }

    warn!(
      target = "nexus::context_registry",
      system_id,
      pid = %self_pid.id(),
      "Sender context registry miss; falling back to root context"
    );

    let actor_system = match with_actor_system(system_id, |sys| sys.clone()) {
      Some(system) => system,
      None => {
        warn!(
          target = "nexus::context_registry",
          system_id,
          pid = %self_pid.id(),
          "ActorSystem not found for provided system_id"
        );
        return None;
      }
    };
    let root = actor_system.get_root_context().await;
    debug!(
      target = "nexus::context_registry",
      system_id,
      pid = %self_pid.id(),
      "Sender context resolved via root context fallback"
    );
    Some(SenderContextHandle::from_root(root))
  }
}

impl From<ContextHandle> for SenderContextHandle {
  fn from(value: ContextHandle) -> Self {
    SenderContextHandle::from_context(value)
  }
}

impl From<RootContext> for SenderContextHandle {
  fn from(value: RootContext) -> Self {
    SenderContextHandle::from_root(value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{ContextHandle, SpawnerPart, StopperPart};
  use crate::actor::core::{Actor, ActorError, Props};
  use crate::actor::supervisor::SupervisorStrategyHandle;
  use async_trait::async_trait;
  use nexus_actor_core_rs::context::CoreActorContextSnapshot;
  use std::sync::Arc;
  use tokio::sync::{Mutex, Notify};

  fn make_snapshot(pid: &CorePid, system_id: crate::actor::actor_system::ActorSystemId) -> CoreSenderSnapshot {
    let context_snapshot = CoreActorContextSnapshot::new(pid.clone(), None, None, None);
    CoreSenderSnapshot::new(context_snapshot, system_id)
  }

  #[derive(Clone, Debug)]
  struct RecordingActor {
    slot: Arc<Mutex<Option<SenderContextHandle>>>,
    ready: Arc<Notify>,
  }

  #[async_trait]
  impl Actor for RecordingActor {
    async fn post_start(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
      let mut guard = self.slot.lock().await;
      *guard = Some(SenderContextHandle::from_context(ctx));
      self.ready.notify_one();
      Ok(())
    }

    async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
      Ok(())
    }

    async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
      None
    }
  }

  #[tokio::test]
  async fn from_core_snapshot_restores_registered_actor_context() {
    let system = ActorSystem::new().await.unwrap();
    let slot = Arc::new(Mutex::new(None));
    let ready = Arc::new(Notify::new());

    let props = Props::from_sync_actor_producer({
      let slot = slot.clone();
      let ready = ready.clone();
      move |_| RecordingActor {
        slot: slot.clone(),
        ready: ready.clone(),
      }
    })
    .await;

    let mut root = system.get_root_context().await;
    let pid = root.spawn(props).await;

    ready.notified().await;

    let sender_handle = {
      let mut guard = slot.lock().await;
      guard
        .take()
        .expect("actor should have captured its sender context handle")
    };

    let snapshot = sender_handle
      .capture_core_snapshot()
      .await
      .expect("sender context snapshot should be captured");

    let restored = SenderContextHandle::from_core_snapshot(&snapshot)
      .await
      .expect("context should be restored from snapshot");

    assert!(restored.capture_core_snapshot().await.is_some());

    root.stop(&pid).await;
  }

  #[tokio::test]
  async fn from_core_snapshot_falls_back_to_root_when_context_missing() {
    let system = ActorSystem::new().await.unwrap();
    let core_pid = CorePid::new("in-memory", "sender-bridge-missing");
    let snapshot = make_snapshot(&core_pid, system.system_id());

    let restored = SenderContextHandle::from_core_snapshot(&snapshot)
      .await
      .expect("root context should be returned when registry is missing entry");

    assert!(restored.capture_core_snapshot().await.is_none());
  }
}

#[async_trait]
impl InfoPart for SenderContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_parent().await,
      SenderContextRef::Root(root) => root.get_parent().await,
    }
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_self_opt().await,
      SenderContextRef::Root(root) => root.get_self_opt().await,
    }
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    match &mut self.0 {
      SenderContextRef::Context(context) => context.set_self(pid).await,
      SenderContextRef::Root(root) => root.set_self(pid).await,
    }
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_actor().await,
      SenderContextRef::Root(root) => root.get_actor().await,
    }
  }

  async fn get_actor_system(&self) -> ActorSystem {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_actor_system().await,
      SenderContextRef::Root(root) => root.get_actor_system().await,
    }
  }
}

#[async_trait]
impl SenderPart for SenderContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    match &self.0 {
      SenderContextRef::Context(context) => {
        if let Some(sender) = context.try_get_sender_opt() {
          Some(sender)
        } else {
          context.get_sender().await
        }
      }
      SenderContextRef::Root(root) => root.get_sender().await,
    }
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    match &mut self.0 {
      SenderContextRef::Context(context) => context.send(pid, message_handle).await,
      SenderContextRef::Root(root) => root.send(pid, message_handle).await,
    }
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    match &mut self.0 {
      SenderContextRef::Context(context) => context.request(pid, message_handle).await,
      SenderContextRef::Root(root) => root.request(pid, message_handle).await,
    }
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    match &mut self.0 {
      SenderContextRef::Context(context) => context.request_with_custom_sender(pid, message_handle, sender).await,
      SenderContextRef::Root(root) => root.request_with_custom_sender(pid, message_handle, sender).await,
    }
  }

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    match &self.0 {
      SenderContextRef::Context(context) => context.request_future(pid, message_handle, timeout).await,
      SenderContextRef::Root(root) => root.request_future(pid, message_handle, timeout).await,
    }
  }
}

#[async_trait]
impl CoreSenderPart for SenderContextHandle {
  async fn get_sender_core(&self) -> Option<CorePid> {
    self.get_sender().await.map(|pid| pid.to_core())
  }

  async fn send_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
    self.send(ExtendedPid::from(pid), message_handle).await
  }

  async fn request_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
    self.request(ExtendedPid::from(pid), message_handle).await
  }

  async fn request_with_custom_sender_core(&mut self, pid: CorePid, message_handle: MessageHandle, sender: CorePid) {
    self
      .request_with_custom_sender(ExtendedPid::from(pid), message_handle, ExtendedPid::from(sender))
      .await
  }

  async fn request_future_core(&self, pid: CorePid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    self
      .request_future(ExtendedPid::from(pid), message_handle, timeout)
      .await
  }
}

#[async_trait]
impl MessagePart for SenderContextHandle {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_message_envelope_opt().await,
      SenderContextRef::Root(root) => root.get_message_envelope_opt().await,
    }
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_message_handle_opt().await,
      SenderContextRef::Root(root) => root.get_message_handle_opt().await,
    }
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    match &self.0 {
      SenderContextRef::Context(context) => context.get_message_header_handle().await,
      SenderContextRef::Root(root) => root.get_message_header_handle().await,
    }
  }
}

impl SenderContext for SenderContextHandle {}
