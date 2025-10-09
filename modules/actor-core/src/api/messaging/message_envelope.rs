use alloc::sync::Arc;

use crate::runtime::context::InternalActorRef;
use crate::runtime::message::DynMessage;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

type SendFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> + Send + Sync;

/// 送信先を抽象化した内部ディスパッチャ。Ask 応答などで利用する。
#[derive(Clone)]
pub struct InternalMessageSender {
  inner: Arc<SendFn>,
  drop_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl core::fmt::Debug for InternalMessageSender {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str("MessageSender(..)")
  }
}

impl InternalMessageSender {
  pub fn new(inner: Arc<SendFn>) -> Self {
    Self { inner, drop_hook: None }
  }

  pub(crate) fn with_drop_hook(inner: Arc<SendFn>, drop_hook: Arc<dyn Fn() + Send + Sync>) -> Self {
    Self {
      inner,
      drop_hook: Some(drop_hook),
    }
  }

  pub fn send_default(&self, message: DynMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.send_with_priority(message, DEFAULT_PRIORITY)
  }

  pub fn send_with_priority(
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

impl Drop for InternalMessageSender {
  fn drop(&mut self) {
    if let Some(hook) = &self.drop_hook {
      hook();
    }
  }
}

/// 型安全なディスパッチャ。内部ディスパッチャをラップし、ユーザーメッセージを自動的にエンベロープ化する。
#[derive(Clone)]
pub struct MessageSender<M>
where
  M: Element, {
  inner: InternalMessageSender,
  _marker: PhantomData<fn(M)>,
}

impl<M> core::fmt::Debug for MessageSender<M>
where
  M: Element,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("MessageSender").finish()
  }
}

impl<M> MessageSender<M>
where
  M: Element,
{
  pub(crate) fn new(inner: InternalMessageSender) -> Self {
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
    self.inner.send_default(dyn_message)
  }

  pub fn dispatch_with_priority(
    &self,
    envelope: MessageEnvelope<M>,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.send_with_priority(dyn_message, priority)
  }

  pub fn internal(&self) -> InternalMessageSender {
    self.inner.clone()
  }

  pub fn into_internal(self) -> InternalMessageSender {
    self.inner
  }
}

/// メッセージに付随するメタデータ（内部表現）。
#[derive(Debug, Clone, Default)]
pub struct InternalMessageMetadata {
  sender: Option<InternalMessageSender>,
  responder: Option<InternalMessageSender>,
}

impl InternalMessageMetadata {
  pub fn new(sender: Option<InternalMessageSender>, responder: Option<InternalMessageSender>) -> Self {
    Self { sender, responder }
  }

  pub fn sender(&self) -> Option<&InternalMessageSender> {
    self.sender.as_ref()
  }

  pub fn sender_cloned(&self) -> Option<InternalMessageSender> {
    self.sender.clone()
  }

  pub fn responder(&self) -> Option<&InternalMessageSender> {
    self.responder.as_ref()
  }

  pub fn responder_cloned(&self) -> Option<InternalMessageSender> {
    self.responder.clone()
  }

  pub fn with_sender(mut self, sender: Option<InternalMessageSender>) -> Self {
    self.sender = sender;
    self
  }

  pub fn with_responder(mut self, responder: Option<InternalMessageSender>) -> Self {
    self.responder = responder;
    self
  }
}

/// 外部 API 向けの型付きメタデータ。
#[derive(Debug, Clone, Default)]
pub struct MessageMetadata {
  inner: InternalMessageMetadata,
}

impl MessageMetadata {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_sender<U>(mut self, sender: MessageSender<U>) -> Self
  where
    U: Element, {
    self.inner = self.inner.with_sender(Some(sender.into_internal()));
    self
  }

  pub fn with_responder<U>(mut self, responder: MessageSender<U>) -> Self
  where
    U: Element, {
    self.inner = self.inner.with_responder(Some(responder.into_internal()));
    self
  }

  pub fn sender_as<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.inner.sender_cloned().map(MessageSender::new)
  }

  pub fn responder_as<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.inner.responder_cloned().map(MessageSender::new)
  }

  pub fn dispatcher_for<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.responder_as::<U>().or_else(|| self.sender_as::<U>())
  }

  pub fn is_empty(&self) -> bool {
    self.inner.sender.is_none() && self.inner.responder.is_none()
  }

  pub(crate) fn into_internal(self) -> InternalMessageMetadata {
    self.inner
  }

  pub(crate) fn from_internal(inner: InternalMessageMetadata) -> Self {
    Self { inner }
  }

  pub fn internal_mut(&mut self) -> &mut InternalMessageMetadata {
    &mut self.inner
  }
}

/// ユーザーメッセージとメタデータを保持するラッパー。
#[derive(Debug, Clone)]
pub struct UserMessage<U> {
  message: U,
  metadata: InternalMessageMetadata,
}

impl<U> UserMessage<U> {
  pub fn new(message: U) -> Self {
    Self {
      message,
      metadata: InternalMessageMetadata::default(),
    }
  }

  pub fn with_metadata(message: U, metadata: MessageMetadata) -> Self {
    Self {
      message,
      metadata: metadata.into_internal(),
    }
  }

  pub fn message(&self) -> &U {
    &self.message
  }

  pub fn metadata(&self) -> MessageMetadata {
    MessageMetadata::from_internal(self.metadata.clone())
  }

  pub fn into_parts(self) -> (U, MessageMetadata) {
    (self.message, MessageMetadata::from_internal(self.metadata))
  }

  pub fn metadata_mut(&mut self) -> &mut InternalMessageMetadata {
    &mut self.metadata
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
