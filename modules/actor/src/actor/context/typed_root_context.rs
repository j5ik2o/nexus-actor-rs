use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{
  ContextSnapshot, InfoPart, MessagePart, RootContext, SenderPart, SpawnerPart, StopperPart, TypedContextSnapshot,
};
use crate::actor::core::{ActorHandle, SpawnError, TypedExtendedPid, TypedProps};
use crate::actor::message::{Message, MessageHandle, ReadonlyMessageHeadersHandle, TypedMessageEnvelope};
use crate::actor::process::actor_future::ActorFuture;
use crate::actor::typed_context::{
  TypedInfoPart, TypedMessagePart, TypedSenderContext, TypedSenderPart, TypedSpawnerContext, TypedSpawnerPart,
  TypedStopperPart,
};
use async_trait::async_trait;
use nexus_message_derive_rs::Message;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct UnitMessage;

#[derive(Debug, Clone)]
pub struct TypedRootContext {
  inner: RootContext,
}

impl TypedRootContext {
  pub fn new(inner: RootContext) -> Self {
    Self { inner }
  }

  pub fn sync_view(&self) -> TypedContextSnapshot<UnitMessage> {
    let snapshot = ContextSnapshot::default()
      .with_actor_system_opt(Some(self.inner.actor_system_snapshot()))
      .with_message_header_opt(Some(ReadonlyMessageHeadersHandle::new_arc(
        self.inner.message_headers_snapshot(),
      )));
    TypedContextSnapshot::new(snapshot)
  }
}

impl TypedSenderContext<UnitMessage> for TypedRootContext {}

#[async_trait]
impl TypedInfoPart<UnitMessage> for TypedRootContext {
  async fn get_parent(&self) -> Option<TypedExtendedPid<UnitMessage>> {
    self.inner.get_parent().await.map(|pid| pid.into())
  }

  async fn get_self_opt(&self) -> Option<TypedExtendedPid<UnitMessage>> {
    self.inner.get_self_opt().await.map(|pid| pid.into())
  }

  async fn set_self(&mut self, pid: TypedExtendedPid<UnitMessage>) {
    self.inner.set_self(pid.into()).await;
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.inner.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.inner.get_actor_system().await
  }
}

#[async_trait]
impl TypedSenderPart<UnitMessage> for TypedRootContext {
  async fn get_sender(&self) -> Option<TypedExtendedPid<UnitMessage>> {
    self.inner.get_sender().await.map(TypedExtendedPid::new)
  }

  async fn send<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self
      .inner
      .send(pid.get_underlying().clone(), MessageHandle::new(message))
      .await;
  }

  async fn request<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self
      .inner
      .request(pid.get_underlying().clone(), MessageHandle::new(message))
      .await;
  }

  async fn request_with_custom_sender<A: Message, B: Message>(
    &mut self,
    pid: TypedExtendedPid<A>,
    message: A,
    sender: TypedExtendedPid<B>,
  ) {
    self
      .inner
      .request_with_custom_sender(
        pid.get_underlying().clone(),
        MessageHandle::new(message),
        sender.get_underlying().clone(),
      )
      .await;
  }

  async fn request_future<A: Message>(&self, pid: TypedExtendedPid<A>, message: A, timeout: Duration) -> ActorFuture {
    self
      .inner
      .request_future(pid.get_underlying().clone(), MessageHandle::new(message), timeout)
      .await
  }
}

#[async_trait]
impl TypedMessagePart<UnitMessage> for TypedRootContext {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<UnitMessage>> {
    self
      .inner
      .get_message_envelope_opt()
      .await
      .map(TypedMessageEnvelope::new)
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.inner.get_message_handle_opt().await
  }

  async fn get_message_opt(&self) -> Option<UnitMessage> {
    self
      .inner
      .get_message_handle_opt()
      .await
      .and_then(|handle| handle.to_typed::<UnitMessage>())
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.inner.get_message_header_handle().await
  }
}

impl TypedSpawnerContext<UnitMessage> for TypedRootContext {}

#[async_trait]
impl TypedSpawnerPart for TypedRootContext {
  async fn spawn<A: Message + Clone>(&mut self, props: TypedProps<A>) -> TypedExtendedPid<A> {
    TypedExtendedPid::new(self.inner.spawn(props.get_underlying().clone()).await)
  }

  async fn spawn_prefix<A: Message + Clone>(&mut self, props: TypedProps<A>, prefix: &str) -> TypedExtendedPid<A> {
    TypedExtendedPid::new(self.inner.spawn_prefix(props.get_underlying().clone(), prefix).await)
  }

  async fn spawn_named<A: Message + Clone>(
    &mut self,
    props: TypedProps<A>,
    id: &str,
  ) -> Result<TypedExtendedPid<A>, SpawnError> {
    self
      .inner
      .spawn_named(props.get_underlying().clone(), id)
      .await
      .map(|pid| TypedExtendedPid::new(pid))
  }
}

#[async_trait]
impl TypedStopperPart<UnitMessage> for TypedRootContext {
  async fn stop(&mut self, pid: &TypedExtendedPid<UnitMessage>) {
    self.inner.stop(pid.get_underlying()).await;
  }

  async fn stop_future_with_timeout(&mut self, pid: &TypedExtendedPid<UnitMessage>, timeout: Duration) -> ActorFuture {
    self.inner.stop_future_with_timeout(pid.get_underlying(), timeout).await
  }

  async fn poison(&mut self, pid: &TypedExtendedPid<UnitMessage>) {
    self.inner.poison(pid.get_underlying()).await;
  }

  async fn poison_future_with_timeout(
    &mut self,
    pid: &TypedExtendedPid<UnitMessage>,
    timeout: Duration,
  ) -> ActorFuture {
    self
      .inner
      .poison_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }
}

#[cfg(test)]
mod tests {
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::message::MessageHeaders;
  use crate::actor::typed_context::TypedContextSyncView;
  use std::sync::Arc;

  #[tokio::test]
  async fn sync_view_exposes_snapshots() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let root = actor_system.get_root_context().await;
    let headers = Arc::new(MessageHeaders::default());
    let typed_root = root.with_headers(headers.clone()).to_typed();

    let snapshot = typed_root.sync_view();
    assert!(snapshot.actor_system_snapshot().is_some());
    assert!(snapshot.message_header_snapshot().is_some());
    assert!(snapshot.parent_snapshot().is_none());
    assert!(snapshot.sender_snapshot().is_none());
  }
}
