use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::ActorError;
use crate::actor::actor::ActorHandle;
use crate::actor::actor::Continuer;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor::Props;
use crate::actor::actor::SpawnError;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::{
  BasePart, Context, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::dispatch::future::ActorFuture;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::ResponseHandle;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};

#[derive(Debug, Clone)]
pub struct ContextHandle(Arc<Mutex<dyn Context>>);

impl ContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn Context>>) -> Self {
    ContextHandle(context)
  }

  pub fn new(c: impl Context + 'static) -> Self {
    ContextHandle(Arc::new(Mutex::new(c)))
  }

  pub(crate) async fn to_actor_context(&self) -> Option<ActorContext> {
    let mg = self.0.lock().await;
    mg.as_any().downcast_ref::<ActorContext>().cloned()
  }
}

impl ExtensionContext for ContextHandle {}

#[async_trait]
impl ExtensionPart for ContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mut mg = self.0.lock().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let mut mg = self.0.lock().await;
    mg.set(ext).await
  }
}

impl SenderContext for ContextHandle {}

#[async_trait]
impl InfoPart for ContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SenderPart for ContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.send(pid, message_handle).await
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.request(pid, message_handle).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.request_with_custom_sender(pid, message_handle, sender).await
  }

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    let mg = self.0.lock().await;
    mg.request_future(pid, message_handle, timeout).await
  }
}

#[async_trait]
impl MessagePart for ContextHandle {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    let mg = self.0.lock().await;
    mg.get_message_envelope_opt().await
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    let mg = self.0.lock().await;
    mg.get_message_handle_opt().await
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let mg = self.0.lock().await;
    mg.get_message_header_handle().await
  }
}

impl ReceiverContext for ContextHandle {}

#[async_trait]
impl ReceiverPart for ContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(envelope).await
  }
}

impl SpawnerContext for ContextHandle {}

#[async_trait]
impl SpawnerPart for ContextHandle {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let mut mg = self.0.lock().await;
    mg.spawn_named(props, id).await
  }
}

#[async_trait]
impl BasePart for ContextHandle {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    let mg = self.0.lock().await;
    mg.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_children().await
  }

  async fn respond(&self, response: ResponseHandle) {
    let mg = self.0.lock().await;
    mg.respond(response).await
  }

  async fn stash(&mut self) {
    let mut mg = self.0.lock().await;
    mg.stash().await
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.un_stash_all().await
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.watch(pid).await
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.unwatch(pid).await
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    let mut mg = self.0.lock().await;
    mg.set_receive_timeout(d).await
  }

  async fn cancel_receive_timeout(&mut self) {
    let mut mg = self.0.lock().await;
    mg.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &ExtendedPid) {
    let mg = self.0.lock().await;
    mg.forward(pid).await
  }

  async fn reenter_after(&self, f: ActorFuture, continuation: Continuer) {
    let mg = self.0.lock().await;
    mg.reenter_after(f, continuation).await
  }
}

#[async_trait]
impl StopperPart for ContextHandle {
  async fn stop(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.stop(pid).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let mut mg = self.0.lock().await;
    mg.stop_future_with_timeout(pid, timeout).await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.poison(pid).await
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let mut mg = self.0.lock().await;
    mg.poison_future_with_timeout(pid, timeout).await
  }
}

impl Context for ContextHandle {}
