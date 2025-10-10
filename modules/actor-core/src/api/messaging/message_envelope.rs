#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;

use crate::runtime::context::InternalActorRef;
use crate::runtime::message::{store_metadata, take_metadata, DynMessage, MetadataKey};
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope, RuntimeBound};
use core::marker::PhantomData;
use core::mem::{forget, ManuallyDrop};
use nexus_utils_core_rs::sync::ArcShared;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

#[cfg(target_has_atomic = "ptr")]
type SendFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type SendFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>;

#[cfg(target_has_atomic = "ptr")]
type DropHookFn = dyn Fn() + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type DropHookFn = dyn Fn();

/// Internal dispatcher that abstracts the sending destination. Used for ask responses and similar purposes.
#[derive(Clone)]
pub struct InternalMessageSender {
  inner: ArcShared<SendFn>,
  drop_hook: Option<ArcShared<DropHookFn>>,
}

impl core::fmt::Debug for InternalMessageSender {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str("MessageSender(..)")
  }
}

impl InternalMessageSender {
  /// Creates a new `InternalMessageSender` with the specified send function.
  ///
  /// # Arguments
  /// * `inner` - Function that executes message sending
  pub fn new(inner: ArcShared<SendFn>) -> Self {
    Self { inner, drop_hook: None }
  }

  /// Creates an `InternalMessageSender` with a drop hook (internal API).
  ///
  /// # Arguments
  /// * `inner` - Function that executes message sending
  /// * `drop_hook` - Hook function executed on drop
  pub(crate) fn with_drop_hook(inner: ArcShared<SendFn>, drop_hook: ArcShared<DropHookFn>) -> Self {
    Self {
      inner,
      drop_hook: Some(drop_hook),
    }
  }

  /// Sends a message with default priority.
  ///
  /// # Arguments
  /// * `message` - Message to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn send_default(&self, message: DynMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.send_with_priority(message, DEFAULT_PRIORITY)
  }

  /// Sends a message with the specified priority.
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `priority` - Message priority
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn send_with_priority(
    &self,
    message: DynMessage,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    (self.inner)(message, priority)
  }

  /// Creates an `InternalMessageSender` from an internal actor reference (internal API).
  ///
  /// # Arguments
  /// * `actor_ref` - Actor reference to send to
  pub(crate) fn from_internal_ref<R>(actor_ref: InternalActorRef<DynMessage, R>) -> Self
  where
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let sender = actor_ref.clone();
    Self::new(ArcShared::from_arc(Arc::new(move |message, priority| {
      sender.try_send_with_priority(message, priority)
    })))
  }
}

impl Drop for InternalMessageSender {
  fn drop(&mut self) {
    if let Some(hook) = &self.drop_hook {
      hook();
    }
  }
}

/// Type-safe dispatcher. Wraps the internal dispatcher and automatically envelopes user messages.
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
  /// Creates a typed `MessageSender` from an internal sender (internal API).
  ///
  /// # Arguments
  /// * `inner` - Internal message sender
  pub(crate) fn new(inner: InternalMessageSender) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  /// Creates a typed `MessageSender` from an internal sender.
  ///
  /// # Arguments
  /// * `inner` - Internal message sender
  pub fn from_internal(inner: InternalMessageSender) -> Self {
    Self::new(inner)
  }

  /// Dispatches a user message.
  ///
  /// # Arguments
  /// * `message` - User message to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn dispatch_user(&self, message: M) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.dispatch_envelope(MessageEnvelope::user(message))
  }

  /// Dispatches a message envelope.
  ///
  /// # Arguments
  /// * `envelope` - Message envelope to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn dispatch_envelope(
    &self,
    envelope: MessageEnvelope<M>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.send_default(dyn_message)
  }

  /// Dispatches a message envelope with the specified priority.
  ///
  /// # Arguments
  /// * `envelope` - Message envelope to send
  /// * `priority` - Message priority
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn dispatch_with_priority(
    &self,
    envelope: MessageEnvelope<M>,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.send_with_priority(dyn_message, priority)
  }

  /// Gets a clone of the internal sender.
  ///
  /// # Returns
  /// Clone of the internal message sender
  pub fn internal(&self) -> InternalMessageSender {
    self.inner.clone()
  }

  /// Converts to the internal sender, transferring ownership.
  ///
  /// # Returns
  /// Internal message sender
  pub fn into_internal(self) -> InternalMessageSender {
    self.inner
  }
}

/// Metadata accompanying a message (internal representation).
#[derive(Debug, Clone, Default)]
pub struct InternalMessageMetadata {
  sender: Option<InternalMessageSender>,
  responder: Option<InternalMessageSender>,
}

impl InternalMessageMetadata {
  /// Creates new metadata with sender and responder.
  ///
  /// # Arguments
  /// * `sender` - Sender's dispatcher (optional)
  /// * `responder` - Responder's dispatcher (optional)
  pub fn new(sender: Option<InternalMessageSender>, responder: Option<InternalMessageSender>) -> Self {
    Self { sender, responder }
  }

  /// Gets a reference to the sender's dispatcher.
  ///
  /// # Returns
  /// `Some(&InternalMessageSender)` if sender exists, `None` otherwise
  pub fn sender(&self) -> Option<&InternalMessageSender> {
    self.sender.as_ref()
  }

  /// Gets a clone of the sender's dispatcher.
  ///
  /// # Returns
  /// `Some(InternalMessageSender)` if sender exists, `None` otherwise
  pub fn sender_cloned(&self) -> Option<InternalMessageSender> {
    self.sender.clone()
  }

  /// Gets a reference to the responder's dispatcher.
  ///
  /// # Returns
  /// `Some(&InternalMessageSender)` if responder exists, `None` otherwise
  pub fn responder(&self) -> Option<&InternalMessageSender> {
    self.responder.as_ref()
  }

  /// Gets a clone of the responder's dispatcher.
  ///
  /// # Returns
  /// `Some(InternalMessageSender)` if responder exists, `None` otherwise
  pub fn responder_cloned(&self) -> Option<InternalMessageSender> {
    self.responder.clone()
  }

  /// Sets the sender and returns self (builder pattern).
  ///
  /// # Arguments
  /// * `sender` - Sender's dispatcher to set
  pub fn with_sender(mut self, sender: Option<InternalMessageSender>) -> Self {
    self.sender = sender;
    self
  }

  /// Sets the responder and returns self (builder pattern).
  ///
  /// # Arguments
  /// * `responder` - Responder's dispatcher to set
  pub fn with_responder(mut self, responder: Option<InternalMessageSender>) -> Self {
    self.responder = responder;
    self
  }
}

/// Typed metadata for the external API.
#[derive(Debug, Clone, Default)]
pub struct MessageMetadata {
  inner: InternalMessageMetadata,
}

impl MessageMetadata {
  /// Creates new empty metadata.
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the sender and returns self (builder pattern).
  ///
  /// # Arguments
  /// * `sender` - Sender's dispatcher to set
  pub fn with_sender<U>(mut self, sender: MessageSender<U>) -> Self
  where
    U: Element, {
    self.inner = self.inner.with_sender(Some(sender.into_internal()));
    self
  }

  /// Sets the responder and returns self (builder pattern).
  ///
  /// # Arguments
  /// * `responder` - Responder's dispatcher to set
  pub fn with_responder<U>(mut self, responder: MessageSender<U>) -> Self
  where
    U: Element, {
    self.inner = self.inner.with_responder(Some(responder.into_internal()));
    self
  }

  /// Gets the sender dispatcher of the specified type.
  ///
  /// # Returns
  /// `Some(MessageSender<U>)` if sender exists, `None` otherwise
  pub fn sender_as<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.inner.sender_cloned().map(MessageSender::new)
  }

  /// Gets the responder dispatcher of the specified type.
  ///
  /// # Returns
  /// `Some(MessageSender<U>)` if responder exists, `None` otherwise
  pub fn responder_as<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.inner.responder_cloned().map(MessageSender::new)
  }

  /// Gets the dispatcher of the specified type (prioritizing responder).
  ///
  /// Returns the responder if it exists, otherwise returns the sender.
  ///
  /// # Returns
  /// `Some(MessageSender<U>)` if dispatcher exists, `None` otherwise
  pub fn dispatcher_for<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.responder_as::<U>().or_else(|| self.sender_as::<U>())
  }

  /// Determines if the metadata is empty.
  ///
  /// # Returns
  /// `true` if neither sender nor responder exists, `false` otherwise
  pub fn is_empty(&self) -> bool {
    self.inner.sender.is_none() && self.inner.responder.is_none()
  }
}

/// Wrapper that holds a user message and metadata.
#[derive(Debug, Clone)]
pub struct UserMessage<U> {
  message: ManuallyDrop<U>,
  metadata_key: Option<MetadataKey>,
}

impl<U> UserMessage<U> {
  /// Creates a new `UserMessage` with only the message.
  ///
  /// # Arguments
  /// * `message` - User message
  pub fn new(message: U) -> Self {
    Self {
      message: ManuallyDrop::new(message),
      metadata_key: None,
    }
  }

  /// Creates a new `UserMessage` with message and metadata.
  ///
  /// If metadata is empty, it is created without metadata.
  ///
  /// # Arguments
  /// * `message` - User message
  /// * `metadata` - Message metadata
  pub fn with_metadata(message: U, metadata: MessageMetadata) -> Self {
    if metadata.is_empty() {
      Self::new(message)
    } else {
      let key = store_metadata(metadata);
      Self {
        message: ManuallyDrop::new(message),
        metadata_key: Some(key),
      }
    }
  }

  /// Gets a reference to the message.
  ///
  /// # Returns
  /// Reference to the user message
  pub fn message(&self) -> &U {
    &*self.message
  }

  /// Gets the metadata key.
  ///
  /// # Returns
  /// `Some(MetadataKey)` if metadata exists, `None` otherwise
  pub fn metadata_key(&self) -> Option<MetadataKey> {
    self.metadata_key
  }

  /// Decomposes into message and metadata key.
  ///
  /// # Returns
  /// Tuple of `(message, metadata key)`
  pub fn into_parts(mut self) -> (U, Option<MetadataKey>) {
    let key = self.metadata_key.take();
    let message = unsafe { ManuallyDrop::take(&mut self.message) };
    forget(self);
    (message, key)
  }
}

impl<U> From<U> for UserMessage<U> {
  fn from(message: U) -> Self {
    Self::new(message)
  }
}

impl<U> Drop for UserMessage<U> {
  fn drop(&mut self) {
    if let Some(key) = self.metadata_key.take() {
      let _ = take_metadata(key);
    }
    unsafe {
      ManuallyDrop::drop(&mut self.message);
    }
  }
}

/// Typed envelope that integrates user messages and system messages.
#[derive(Debug, Clone)]
pub enum MessageEnvelope<U> {
  /// Variant that holds a user message.
  User(UserMessage<U>),
  /// Variant that holds a system message.
  System(SystemMessage),
}

impl<U> MessageEnvelope<U>
where
  U: Element,
{
  /// Creates an envelope for a user message.
  ///
  /// # Arguments
  /// * `message` - User message
  pub fn user(message: U) -> Self {
    MessageEnvelope::User(UserMessage::new(message))
  }

  /// Creates an envelope for a user message with metadata.
  ///
  /// # Arguments
  /// * `message` - User message
  /// * `metadata` - Message metadata
  pub fn user_with_metadata(message: U, metadata: MessageMetadata) -> Self {
    MessageEnvelope::User(UserMessage::with_metadata(message, metadata))
  }
}

impl<U> Element for MessageEnvelope<U> where U: Element {}
