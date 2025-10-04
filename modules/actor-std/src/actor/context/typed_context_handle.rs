use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::{ActorContext, ContextBorrow};
use crate::actor::context::{
  BasePart, ContextCellStats, ContextHandle, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverPart,
  SenderPart, SpawnerPart, StopperPart, TypedContextSnapshot,
};
use crate::actor::core::{ActorError, ActorHandle, Continuer, ExtendedPid, SpawnError, TypedExtendedPid, TypedProps};
use crate::actor::message::{
  Message, MessageHandle, ReadonlyMessageHeadersHandle, ResponseHandle, TypedMessageEnvelope,
};
use crate::actor::process::actor_future::ActorFuture;
use crate::actor::typed_context::{
  TypedContext, TypedInfoPart, TypedMessagePart, TypedReceiverContext, TypedReceiverPart, TypedSenderContext,
  TypedSenderPart, TypedSpawnerContext, TypedSpawnerPart, TypedStopperPart,
};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use async_trait::async_trait;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub struct TypedContextHandle<M: Message> {
  underlying: ContextHandle,
  phantom_data: PhantomData<M>,
}

impl<M: Message> Clone for TypedContextHandle<M> {
  fn clone(&self) -> Self {
    Self {
      underlying: self.underlying.clone(),
      phantom_data: PhantomData,
    }
  }
}

impl<M: Message> TypedContextHandle<M> {
  pub fn new(underlying: ContextHandle) -> Self {
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }

  pub fn get_underlying(&self) -> &ContextHandle {
    &self.underlying
  }

  pub fn actor_context_arc(&self) -> Option<Arc<ActorContext>> {
    self.underlying.actor_context_arc()
  }

  pub fn with_actor_borrow<R, F>(&self, f: F) -> Option<R>
  where
    F: for<'a> FnOnce(ContextBorrow<'a>) -> R, {
    self.underlying.with_actor_borrow(f)
  }

  pub fn context_cell_stats(&self) -> ContextCellStats {
    self.underlying.context_cell_stats()
  }

  pub fn try_message_envelope(&self) -> Option<TypedMessageEnvelope<M>>
  where
    M: Clone, {
    self
      .underlying
      .try_get_message_envelope_opt()
      .map(TypedMessageEnvelope::new)
  }

  pub fn try_message_handle(&self) -> Option<MessageHandle> {
    self.underlying.try_get_message_handle_opt()
  }

  pub fn try_message_opt(&self) -> Option<M>
  where
    M: Clone, {
    self.try_message_handle().and_then(|handle| handle.to_typed::<M>())
  }

  pub fn try_message_header(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.underlying.try_get_message_header_handle()
  }

  pub fn try_sender(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.try_get_sender_opt().map(|pid| pid.into())
  }

  pub fn sync_view(&self) -> TypedContextSnapshot<M> {
    TypedContextSnapshot::new(self.underlying.snapshot())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::actor_context::ActorContext;
  use crate::actor::context::context_handle::ContextHandle;
  use crate::actor::core::{ActorError, Props};
  use crate::actor::message::{Message, MessageEnvelope};
  use crate::actor::typed_context::TypedContextSyncView;
  use std::any::Any;

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct TestMessage;

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().downcast_ref::<Self>().is_some()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }

    fn get_type_name(&self) -> String {
      "TestMessage".to_string()
    }
  }

  #[tokio::test]
  async fn try_typed_message_snapshot() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let props = Props::from_async_actor_receiver(|_ctx| async { Ok::<(), ActorError>(()) }).await;
    let actor_context = ActorContext::new(actor_system.clone(), props, None).await;

    let sender_pid = actor_system.new_local_pid("sender").await;
    let envelope = MessageEnvelope::new(MessageHandle::new(TestMessage.clone())).with_sender(sender_pid.clone());
    actor_context
      .inject_message_for_test(MessageHandle::new(envelope))
      .await;

    let typed_handle = TypedContextHandle::<TestMessage>::new(ContextHandle::new(actor_context.clone()));
    let snapshot = typed_handle.sync_view();

    assert_eq!(snapshot.message_snapshot(), Some(TestMessage));
    assert!(snapshot.message_handle_snapshot().is_some());
    assert_eq!(
      snapshot.sender_snapshot().map(|pid| pid.get_underlying().clone()),
      Some(sender_pid.clone()),
    );
    assert!(snapshot.actor_system_snapshot().is_some());

    actor_context.clear_message_for_test().await;
    assert_eq!(snapshot.message_snapshot(), Some(TestMessage));

    let refreshed = typed_handle.sync_view();
    assert!(refreshed.message_snapshot().is_none());
  }
}

impl<M: Message> ExtensionContext for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> ExtensionPart for TypedContextHandle<M> {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    self.underlying.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    self.underlying.set(ext).await
  }
}

impl<M: Message + Clone> TypedSenderContext<M> for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> TypedInfoPart<M> for TypedContextHandle<M> {
  async fn get_parent(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.get_parent().await.map(|pid| pid.into())
  }

  async fn get_self_opt(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.get_self_opt().await.map(|pid| pid.into())
  }

  async fn set_self(&mut self, pid: TypedExtendedPid<M>) {
    self.underlying.set_self(pid.into()).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.underlying.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.underlying.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> TypedSenderPart<M> for TypedContextHandle<M> {
  async fn get_sender(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.get_sender().await.map(|pid| pid.into())
  }

  async fn send<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self.underlying.send(pid.into(), MessageHandle::new(message)).await
  }

  async fn request<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self.underlying.request(pid.into(), MessageHandle::new(message)).await
  }

  async fn request_with_custom_sender<A: Message, B: Message>(
    &mut self,
    pid: TypedExtendedPid<A>,
    message: A,
    sender: TypedExtendedPid<B>,
  ) {
    self
      .underlying
      .request_with_custom_sender(pid.into(), MessageHandle::new(message), sender.into())
      .await
  }

  async fn request_future<A: Message>(&self, pid: TypedExtendedPid<A>, message: A, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .request_future(pid.into(), MessageHandle::new(message), timeout)
      .await
  }
}

#[async_trait]
impl<M: Message + Clone> TypedMessagePart<M> for TypedContextHandle<M> {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>> {
    self
      .underlying
      .get_message_envelope_opt()
      .await
      .map(|envelope| TypedMessageEnvelope::new(envelope))
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.underlying.get_message_handle_opt().await
  }

  async fn get_message_opt(&self) -> Option<M> {
    self
      .underlying
      .get_message_handle_opt()
      .await
      .and_then(|handle| handle.to_typed::<M>())
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.underlying.get_message_header_handle().await
  }
}

impl<M: Message + Clone> TypedReceiverContext<M> for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> TypedReceiverPart<M> for TypedContextHandle<M> {
  async fn receive(&mut self, envelope: TypedMessageEnvelope<M>) -> Result<(), ActorError> {
    self.underlying.receive(envelope.into()).await
  }
}

impl<M: Message> TypedSpawnerContext<M> for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> TypedSpawnerPart for TypedContextHandle<M> {
  async fn spawn<A: Message + Clone>(&mut self, props: TypedProps<A>) -> TypedExtendedPid<A> {
    self.underlying.spawn(props.into()).await.into()
  }

  async fn spawn_prefix<A: Message + Clone>(&mut self, props: TypedProps<A>, prefix: &str) -> TypedExtendedPid<A> {
    self.underlying.spawn_prefix(props.into(), prefix).await.into()
  }

  async fn spawn_named<A: Message + Clone>(
    &mut self,
    props: TypedProps<A>,
    id: &str,
  ) -> Result<TypedExtendedPid<A>, SpawnError> {
    self
      .underlying
      .spawn_named(props.into(), id)
      .await
      .map(|pid| pid.into())
  }
}

#[async_trait]
impl<M: Message> BasePart for TypedContextHandle<M> {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.underlying.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    self.underlying.get_children().await
  }

  async fn respond(&self, response: ResponseHandle) {
    self.underlying.respond(response).await
  }

  async fn stash(&mut self) {
    self.underlying.stash().await
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    self.underlying.un_stash_all().await
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    self.underlying.watch_core(&pid.to_core()).await
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    self.underlying.unwatch_core(&pid.to_core()).await
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    self.underlying.set_receive_timeout(d).await
  }

  async fn cancel_receive_timeout(&mut self) {
    self.underlying.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &ExtendedPid) {
    self.underlying.forward_core(&pid.to_core()).await
  }

  async fn reenter_after(&self, f: ActorFuture, continuation: Continuer) {
    self.underlying.reenter_after(f, continuation).await
  }
}

#[async_trait]
impl<M: Message> TypedStopperPart<M> for TypedContextHandle<M> {
  async fn stop(&mut self, pid: &TypedExtendedPid<M>) {
    self.underlying.stop(pid.get_underlying()).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .stop_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }

  async fn poison(&mut self, pid: &TypedExtendedPid<M>) {
    self.underlying.poison(pid.get_underlying()).await
  }

  async fn poison_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .poison_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }
}

impl<M: Message + Clone> TypedContext<M> for TypedContextHandle<M> {}
