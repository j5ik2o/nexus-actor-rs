use async_trait::async_trait;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::root_context::RootContext;
use crate::actor::context::{CoreSenderPart, InfoPart, MessagePart, SenderContext, SenderPart};
use crate::actor::core::ActorHandle;
use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::process::actor_future::ActorFuture;
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
