use alloc::sync::Arc;

use crate::runtime::context::InternalActorRef;
use crate::runtime::message::DynMessage;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

type DispatchFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> + Send + Sync;

/// 送信先を抽象化した内部ディスパッチャ。Ask 応答などで利用する。
#[derive(Clone)]
pub struct InternalMessageDispatcher {
  inner: Arc<DispatchFn>,
  drop_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl core::fmt::Debug for InternalMessageDispatcher {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str("MessageDispatcher(..)")
  }
}

impl InternalMessageDispatcher {
  pub fn new(inner: Arc<DispatchFn>) -> Self {
    Self { inner, drop_hook: None }
  }

  pub(crate) fn with_drop_hook(inner: Arc<DispatchFn>, drop_hook: Arc<dyn Fn() + Send + Sync>) -> Self {
    Self {
      inner,
      drop_hook: Some(drop_hook),
    }
  }

  pub fn dispatch_default(&self, message: DynMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.dispatch_with_priority(message, DEFAULT_PRIORITY)
  }

  pub fn dispatch_with_priority(
    &self,
    message: DynMessage,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    (self.inner)(message, priority)
  }

  pub(crate) fn from_internal_ref<R>(actor_ref: InternalActorRef<DynMessage, R>) -> Self
  where
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let sender = actor_ref.clone();
    Self::new(Arc::new(move |message, priority| {
      sender.try_send_with_priority(message, priority)
    }))
  }
}

impl Drop for InternalMessageDispatcher {
  fn drop(&mut self) {
    if let Some(hook) = &self.drop_hook {
      hook();
    }
  }
}

/// 型安全なディスパッチャ。内部ディスパッチャをラップし、ユーザーメッセージを自動的にエンベロープ化する。
#[derive(Clone)]
pub struct MessageDispatcher<M>
where
  M: Element, {
  inner: InternalMessageDispatcher,
  _marker: PhantomData<fn(M)>,
}

impl<M> core::fmt::Debug for MessageDispatcher<M>
where
  M: Element,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("MessageDispatcher").finish()
  }
}

impl<M> MessageDispatcher<M>
where
  M: Element,
{
  pub(crate) fn new(inner: InternalMessageDispatcher) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  pub fn dispatch_user(&self, message: M) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.dispatch_envelope(MessageEnvelope::user(message))
  }

  pub fn dispatch_envelope(
    &self,
    envelope: MessageEnvelope<M>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.dispatch_default(dyn_message)
  }

  pub fn dispatch_with_priority(
    &self,
    envelope: MessageEnvelope<M>,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.dispatch_with_priority(dyn_message, priority)
  }

  pub fn internal(&self) -> InternalMessageDispatcher {
    self.inner.clone()
  }

  pub fn into_internal(self) -> InternalMessageDispatcher {
    self.inner
  }
}

/// メッセージに付随するメタデータ。Ask 系 API が Sender/Responder をここに格納する。
#[derive(Debug, Clone, Default)]
pub struct MessageMetadata {
  sender: Option<InternalMessageDispatcher>,
  responder: Option<InternalMessageDispatcher>,
}

impl MessageMetadata {
  pub fn new(sender: Option<InternalMessageDispatcher>, responder: Option<InternalMessageDispatcher>) -> Self {
    Self { sender, responder }
  }

  pub fn sender(&self) -> Option<&InternalMessageDispatcher> {
    self.sender.as_ref()
  }

  pub fn sender_cloned(&self) -> Option<InternalMessageDispatcher> {
    self.sender.clone()
  }

  pub fn responder(&self) -> Option<&InternalMessageDispatcher> {
    self.responder.as_ref()
  }

  pub fn responder_cloned(&self) -> Option<InternalMessageDispatcher> {
    self.responder.clone()
  }

  pub fn with_sender(mut self, sender: Option<InternalMessageDispatcher>) -> Self {
    self.sender = sender;
    self
  }

  pub fn with_responder(mut self, responder: Option<InternalMessageDispatcher>) -> Self {
    self.responder = responder;
    self
  }
}

/// ユーザーメッセージとメタデータを保持するラッパー。
#[derive(Debug, Clone)]
pub struct UserMessage<U> {
  message: U,
  metadata: MessageMetadata,
}

impl<U> UserMessage<U> {
  pub fn new(message: U) -> Self {
    Self {
      message,
      metadata: MessageMetadata::default(),
    }
  }

  pub fn with_metadata(message: U, metadata: MessageMetadata) -> Self {
    Self { message, metadata }
  }

  pub fn message(&self) -> &U {
    &self.message
  }

  pub fn metadata(&self) -> &MessageMetadata {
    &self.metadata
  }

  pub fn into_parts(self) -> (U, MessageMetadata) {
    (self.message, self.metadata)
  }
}

impl<U> From<U> for UserMessage<U> {
  fn from(message: U) -> Self {
    Self::new(message)
  }
}

/// Typed envelope combining user and system messages.
#[derive(Debug, Clone)]
pub enum MessageEnvelope<U> {
  User(UserMessage<U>),
  System(SystemMessage),
}

impl<U> MessageEnvelope<U>
where
  U: Element,
{
  pub fn user(message: U) -> Self {
    MessageEnvelope::User(UserMessage::new(message))
  }

  pub fn user_with_metadata(message: U, metadata: MessageMetadata) -> Self {
    MessageEnvelope::User(UserMessage::with_metadata(message, metadata))
  }
}

impl<U> Element for MessageEnvelope<U> where U: Element {}
