use crate::actor::actor::ActorError;
use crate::actor::actor::ActorHandle;
use crate::actor::actor::Continuer;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor::Props;
use crate::actor::actor::SpawnError;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{
  BasePart, Context, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::future::{ActorFuture, ActorFutureProcess};
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::ResponseHandle;
use crate::actor::process::Process;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use async_trait::async_trait;
use std::any::Any;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MockContext {
  system: ActorSystem,
}

impl MockContext {
  pub fn new(system: ActorSystem) -> Self {
    Self { system }
  }
}

impl ExtensionContext for MockContext {}

#[async_trait]
impl ExtensionPart for MockContext {
  async fn get(&mut self, _: ContextExtensionId) -> Option<ContextExtensionHandle> {
    None
  }

  async fn set(&mut self, _: ContextExtensionHandle) {}
}

impl SenderContext for MockContext {}

#[async_trait]
impl InfoPart for MockContext {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    None
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    None
  }

  async fn set_self(&mut self, _: ExtendedPid) {}

  async fn get_actor(&self) -> Option<ActorHandle> {
    None
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.system.clone()
  }
}

#[async_trait]
impl SenderPart for MockContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    None
  }

  async fn send(&mut self, _: ExtendedPid, _: MessageHandle) {}

  async fn request(&mut self, _: ExtendedPid, _: MessageHandle) {}

  async fn request_with_custom_sender(&mut self, _: ExtendedPid, _: MessageHandle, _: ExtendedPid) {}

  async fn request_future(&self, _: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    let process = ActorFutureProcess::new(self.system.clone(), timeout).await;
    process.send_user_message(None, message_handle).await;
    process.get_future().await
  }
}

#[async_trait]
impl MessagePart for MockContext {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    None
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    None
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    None
  }
}

impl ReceiverContext for MockContext {}

#[async_trait]
impl ReceiverPart for MockContext {
  async fn receive(&mut self, _: MessageEnvelope) -> Result<(), ActorError> {
    Ok(())
  }
}

impl SpawnerContext for MockContext {}

#[async_trait]
impl SpawnerPart for MockContext {
  async fn spawn(&mut self, _: Props) -> ExtendedPid {
    todo!()
  }

  async fn spawn_prefix(&mut self, _: Props, _: &str) -> ExtendedPid {
    todo!()
  }

  async fn spawn_named(&mut self, _: Props, _: &str) -> Result<ExtendedPid, SpawnError> {
    todo!()
  }
}

#[async_trait]
impl BasePart for MockContext {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    todo!()
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    todo!()
  }

  async fn respond(&self, _: ResponseHandle) {
    todo!()
  }

  async fn stash(&mut self) {
    todo!()
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    todo!()
  }

  async fn watch(&mut self, _: &ExtendedPid) {
    todo!()
  }

  async fn unwatch(&mut self, _: &ExtendedPid) {
    todo!()
  }

  async fn set_receive_timeout(&mut self, _: &Duration) {
    todo!()
  }

  async fn cancel_receive_timeout(&mut self) {
    todo!()
  }

  async fn forward(&self, _: &ExtendedPid) {
    todo!()
  }

  async fn reenter_after(&self, _: ActorFuture, _: Continuer) {
    todo!()
  }
}

#[async_trait]
impl StopperPart for MockContext {
  async fn stop(&mut self, _: &ExtendedPid) {}

  async fn stop_future_with_timeout(&mut self, _: &ExtendedPid, _: Duration) -> ActorFuture {
    todo!()
  }

  async fn poison(&mut self, _: &ExtendedPid) {}

  async fn poison_future_with_timeout(&mut self, _: &ExtendedPid, _: Duration) -> ActorFuture {
    todo!()
  }
}

impl Context for MockContext {}
