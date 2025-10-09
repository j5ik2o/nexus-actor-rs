use crate::runtime::context::InternalActorRef;
use crate::runtime::message::DynMessage;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::{ask::create_ask_handles, ask_with_timeout, AskError, AskFuture, AskResult, AskTimeoutFuture};
use crate::api::{InternalMessageSender, MessageEnvelope, MessageMetadata, MessageSender};

/// Typed actor reference.
///
/// Used to send user messages and system messages to the mailbox,
/// and to receive responses via `ask`-style APIs.
#[derive(Clone)]
pub struct ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: InternalActorRef<DynMessage, R>,
  _marker: PhantomData<U>,
}

impl<U, R> ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// Creates a new `ActorRef` from an internal reference.
  ///
  /// # Arguments
  /// * `inner` - Internal actor reference
  pub(crate) fn new(inner: InternalActorRef<DynMessage, R>) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  /// Wraps a user message into a dynamic message.
  ///
  /// # Arguments
  /// * `message` - User message to wrap
  pub(crate) fn wrap_user(message: U) -> DynMessage {
    DynMessage::new(MessageEnvelope::user(message))
  }

  /// Wraps a user message with metadata into a dynamic message.
  ///
  /// # Arguments
  /// * `message` - User message to wrap
  /// * `metadata` - Metadata attached to the message
  pub(crate) fn wrap_user_with_metadata(message: U, metadata: MessageMetadata) -> DynMessage {
    DynMessage::new(MessageEnvelope::user_with_metadata(message, metadata))
  }

  /// Sends an already wrapped message with priority.
  ///
  /// # Arguments
  /// * `dyn_message` - Dynamic message
  /// * `priority` - Message priority
  fn send_envelope(
    &self,
    dyn_message: DynMessage,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(dyn_message, priority)
  }

  /// Sends a message with metadata (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `metadata` - Metadata attached to the message
  pub(crate) fn tell_with_metadata(
    &self,
    message: U,
    metadata: MessageMetadata,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = Self::wrap_user_with_metadata(message, metadata);
    self.send_envelope(dyn_message, DEFAULT_PRIORITY)
  }

  /// Sends a message (Fire-and-Forget).
  ///
  /// # Arguments
  /// * `message` - Message to send
  ///
  /// # Returns
  /// `Ok(())` on success, error on failure
  pub fn tell(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self
      .inner
      .try_send_with_priority(Self::wrap_user(message), DEFAULT_PRIORITY)
  }

  /// Sends a message with specified priority.
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `priority` - Message priority (lower value = higher priority)
  ///
  /// # Returns
  /// `Ok(())` on success, error on failure
  pub fn tell_with_priority(&self, message: U, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(Self::wrap_user(message), priority)
  }

  /// Sends a system message.
  ///
  /// # Arguments
  /// * `message` - System message to send
  ///
  /// # Returns
  /// `Ok(())` on success, error on failure
  pub fn send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.try_send_envelope(envelope)
  }

  /// Converts this actor reference to a message dispatcher.
  ///
  /// # Returns
  /// Message dispatcher for sending messages
  pub fn to_dispatcher(&self) -> MessageSender<U>
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let internal = InternalMessageSender::from_internal_ref(self.inner.clone());
    MessageSender::new(internal)
  }

  /// Sends a request with specified sender actor (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `sender` - Sender actor reference
  #[allow(dead_code)]
  pub(crate) fn request_from<S>(
    &self,
    message: U,
    sender: &ActorRef<S, R>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_with_dispatcher(message, sender.to_dispatcher())
  }

  /// Sends a request with specified dispatcher (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `sender` - Sender dispatcher
  #[allow(dead_code)]
  pub(crate) fn request_with_dispatcher<S>(
    &self,
    message: U,
    sender: MessageSender<S>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    S: Element, {
    let metadata = MessageMetadata::new().with_sender(sender);
    self.tell_with_metadata(message, metadata)
  }

  /// Generates a response channel internally, sends `message`, and returns `AskFuture` (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  ///
  /// # Returns
  /// `AskFuture` awaiting response
  pub(crate) fn request_future<Resp>(&self, message: U) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// Issues `ask` with specified sender actor reference (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `sender` - Sender actor reference
  #[allow(dead_code)]
  pub(crate) fn request_future_from<Resp, S>(&self, message: U, sender: &ActorRef<S, R>) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_future_with_dispatcher(message, sender.to_dispatcher())
  }

  /// Issues `ask` with arbitrary dispatcher as sender (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `sender` - Sender dispatcher
  #[allow(dead_code)]
  pub(crate) fn request_future_with_dispatcher<Resp, S>(
    &self,
    message: U,
    sender: MessageSender<S>,
  ) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    S: Element, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_sender(sender).with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// Issues `ask` with timeout (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `timeout` - Future for timeout control
  #[allow(dead_code)]
  pub(crate) fn request_future_with_timeout<Resp, TFut>(
    &self,
    message: U,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_responder(responder);
    match self.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// Issues `ask` with timeout and specified sender (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `sender` - Sender actor reference
  /// * `timeout` - Future for timeout control
  #[allow(dead_code)]
  pub(crate) fn request_future_with_timeout_from<Resp, S, TFut>(
    &self,
    message: U,
    sender: &ActorRef<S, R>,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    S: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_future_with_timeout_dispatcher(message, sender.to_dispatcher(), timeout)
  }

  /// Issues `ask` with timeout and specified dispatcher (internal API).
  ///
  /// # Arguments
  /// * `message` - Message to send
  /// * `sender` - Sender dispatcher
  /// * `timeout` - Future for timeout control
  #[allow(dead_code)]
  pub(crate) fn request_future_with_timeout_dispatcher<Resp, S, TFut>(
    &self,
    message: U,
    sender: MessageSender<S>,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    S: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_sender(sender).with_responder(responder);
    match self.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// Constructs a message using a factory function and sends it with `ask` pattern.
  ///
  /// The factory receives a response dispatcher to construct the message.
  ///
  /// # Arguments
  /// * `factory` - Function that receives response dispatcher and generates message
  ///
  /// # Returns
  /// `AskFuture` awaiting response
  pub fn ask_with<Resp, F>(&self, factory: F) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    F: FnOnce(MessageSender<Resp>) -> U, {
    let (future, responder) = create_ask_handles::<Resp>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::new().with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// Issues `ask` using a factory function with timeout.
  ///
  /// # Arguments
  /// * `factory` - Function that receives response dispatcher and generates message
  /// * `timeout` - Future for timeout control
  ///
  /// # Returns
  /// `AskTimeoutFuture` with timeout control
  pub fn ask_with_timeout<Resp, F, TFut>(&self, factory: F, timeout: TFut) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    F: FnOnce(MessageSender<Resp>) -> U,
    TFut: Future<Output = ()> + Unpin, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::new().with_responder(responder);
    match self.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }
}
