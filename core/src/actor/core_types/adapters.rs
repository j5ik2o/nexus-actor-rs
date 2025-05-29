use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart, StopperPart};
use crate::actor::core::ExtendedPid;
use crate::actor::core_types::{ActorRef, ActorRefError, BaseContext};
use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

/// Adapter to make ExtendedPid implement ActorRef
pub struct PidActorRef {
  pid: ExtendedPid,
  actor_system: ActorSystem,
}

impl PidActorRef {
  pub fn new(pid: ExtendedPid, actor_system: ActorSystem) -> Self {
    PidActorRef { pid, actor_system }
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
    self.pid.send_user_message(self.actor_system.clone(), message).await
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
}

impl ContextAdapter {
  pub fn new(context: ContextHandle) -> Self {
    ContextAdapter { context }
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
    // Since this is async, we'll need to block on the future
    // In a real implementation, this would be handled differently
    let handle = tokio::runtime::Handle::current();
    let pid = handle.block_on(async { self.context.get_self().await });
    let actor_system = handle.block_on(async { self.context.get_actor_system().await });
    Box::new(PidActorRef::new(pid, actor_system))
  }

  fn parent_ref(&self) -> Option<Box<dyn ActorRef>> {
    let handle = tokio::runtime::Handle::current();
    let parent_opt = handle.block_on(async { self.context.get_parent().await });
    parent_opt.map(|pid| {
      let actor_system = handle.block_on(async { self.context.get_actor_system().await });
      Box::new(PidActorRef::new(pid, actor_system)) as Box<dyn ActorRef>
    })
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
    sender_opt.map(|pid| {
      let actor_system = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async { self.context.get_actor_system().await })
      });
      Box::new(PidActorRef::new(pid, actor_system)) as Box<dyn ActorRef>
    })
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
  fn adapt_context(&self, context: ContextHandle) -> Box<dyn BaseContext> {
    Box::new(ContextAdapter::new(context))
  }
}
