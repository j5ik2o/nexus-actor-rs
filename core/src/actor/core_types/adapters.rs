use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart, StopperPart};
use crate::actor::core::ExtendedPid;
use crate::actor::core_types::{ActorRef, ActorRefError, BaseContext};
use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

/// Adapter to make ExtendedPid implement ActorRef
#[derive(Clone)]
pub struct PidActorRef {
  pid: ExtendedPid,
  actor_system: WeakActorSystem,
}

impl PidActorRef {
  pub fn new(pid: ExtendedPid, actor_system: ActorSystem) -> Self {
    PidActorRef {
      pid,
      actor_system: actor_system.downgrade(),
    }
  }

  fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before PidActorRef")
  }
}

impl Debug for PidActorRef {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "PidActorRef({:?})", self.pid)
  }
}

#[async_trait]
impl ActorRef for PidActorRef {
  fn get_id(&self) -> String {
    self.pid.id().to_string()
  }

  fn get_address(&self) -> String {
    self.pid.address().to_string()
  }

  async fn tell(&self, message: MessageHandle) {
    self.pid.send_user_message(self.actor_system(), message).await
  }

  async fn request(&self, _message: MessageHandle, _timeout: Duration) -> Result<MessageHandle, ActorRefError> {
    // For now, this is a placeholder. In a real implementation,
    // this would create a future and wait for response using the actor system
    todo!("Implement request for PidActorRef")
  }

  fn is_alive(&self) -> bool {
    // For now, we assume it's always alive
    // This would need to check the process registry in a real implementation
    true
  }
}

/// Adapter to make ContextHandle work with BaseContext
pub struct ContextAdapter {
  context: ContextHandle,
  self_ref: Option<PidActorRef>,
  parent_ref: Option<PidActorRef>,
}

impl ContextAdapter {
  pub async fn new(context: ContextHandle) -> Self {
    let actor_system = context.get_actor_system().await;
    let self_pid = context.get_self_opt().await;
    let parent_pid = context.get_parent().await;

    let self_ref = self_pid.clone().map(|pid| PidActorRef::new(pid, actor_system.clone()));
    let parent_ref = parent_pid
      .clone()
      .map(|pid| PidActorRef::new(pid, actor_system.clone()));

    ContextAdapter {
      context,
      self_ref,
      parent_ref,
    }
  }

  pub fn get_context(&self) -> &ContextHandle {
    &self.context
  }
}

impl Debug for ContextAdapter {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextAdapter")
  }
}

#[async_trait]
impl BaseContext for ContextAdapter {
  fn self_ref(&self) -> Box<dyn ActorRef> {
    let pid_ref = self
      .self_ref
      .as_ref()
      .expect("ContextAdapter: self reference is unavailable")
      .clone();
    Box::new(pid_ref)
  }

  fn parent_ref(&self) -> Option<Box<dyn ActorRef>> {
    self
      .parent_ref
      .as_ref()
      .map(|pid_ref| Box::new(pid_ref.clone()) as Box<dyn ActorRef>)
  }

  async fn send(&self, target: &dyn ActorRef, message: MessageHandle) {
    // We need to convert the ActorRef back to ExtendedPid
    // For now, we'll create a new ExtendedPid from the address and id
    let pid = crate::generated::actor::Pid::new(&target.get_address(), &target.get_id());
    let extended_pid = ExtendedPid::new(pid);

    // Clone the context to make it mutable
    let mut context_clone = self.context.clone();
    context_clone.send(extended_pid, message).await;
  }

  async fn get_message(&self) -> MessageHandle {
    self.context.get_message_handle().await
  }

  async fn get_sender(&self) -> Option<Box<dyn ActorRef>> {
    let sender_opt = self.context.get_sender().await;
    if let Some(pid) = sender_opt {
      let actor_system = self.context.get_actor_system().await;
      Some(Box::new(PidActorRef::new(pid, actor_system)) as Box<dyn ActorRef>)
    } else {
      None
    }
  }

  async fn spawn_child(
    &self,
    name: &str,
    _factory: Box<dyn crate::actor::core_types::ActorFactory>,
  ) -> Box<dyn ActorRef> {
    // For now, we'll use the existing Props system with a dummy actor
    // In a real implementation, we'd use the factory to create the actor
    use crate::actor::core::ActorError;

    let props = crate::actor::core::Props::from_async_actor_receiver(|_ctx| async move {
      // Placeholder actor that does nothing
      Ok::<(), ActorError>(())
    })
    .await;

    let mut context_clone = self.context.clone();
    let pid = context_clone.spawn_named(props, name).await.unwrap();
    let actor_system = self.context.get_actor_system().await;
    Box::new(PidActorRef::new(pid, actor_system))
  }

  async fn stop_child(&self, child: &dyn ActorRef) {
    let pid = crate::generated::actor::Pid::new(&child.get_address(), &child.get_id());
    let extended_pid = ExtendedPid::new(pid);

    let mut context_clone = self.context.clone();
    context_clone.stop(&extended_pid).await;
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

/// Bridge trait to connect old Actor trait with new BaseActor trait
#[async_trait]
pub trait ActorBridge: crate::actor::core::Actor {
  /// Convert ContextHandle to BaseContext for use with new traits
  async fn adapt_context(&self, context: ContextHandle) -> Box<dyn BaseContext> {
    Box::new(ContextAdapter::new(context).await)
  }
}
