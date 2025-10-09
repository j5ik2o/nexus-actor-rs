use crate::runtime::context::InternalActorRef;
use crate::runtime::message::DynMessage;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::{ask::create_ask_handles, ask_with_timeout, AskError, AskFuture, AskResult, AskTimeoutFuture};
use crate::api::{InternalMessageSender, MessageEnvelope, MessageMetadata, MessageSender};

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
  pub(crate) fn new(inner: InternalActorRef<DynMessage, R>) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  pub(crate) fn wrap_user(message: U) -> DynMessage {
    DynMessage::new(MessageEnvelope::user(message))
  }

  pub(crate) fn wrap_user_with_metadata(message: U, metadata: MessageMetadata) -> DynMessage {
    DynMessage::new(MessageEnvelope::user_with_metadata(message, metadata))
  }

  fn send_envelope(
    &self,
    dyn_message: DynMessage,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(dyn_message, priority)
  }

  pub(crate) fn tell_with_metadata(
    &self,
    message: U,
    metadata: MessageMetadata,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = Self::wrap_user_with_metadata(message, metadata);
    self.send_envelope(dyn_message, DEFAULT_PRIORITY)
  }

  pub fn tell(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self
      .inner
      .try_send_with_priority(Self::wrap_user(message), DEFAULT_PRIORITY)
  }

  pub fn tell_with_priority(&self, message: U, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(Self::wrap_user(message), priority)
  }

  pub fn send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.try_send_envelope(envelope)
  }

  pub fn to_dispatcher(&self) -> MessageSender<U>
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let internal = InternalMessageSender::from_internal_ref(self.inner.clone());
    MessageSender::new(internal)
  }

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

  pub(crate) fn request_future<Resp>(&self, message: U) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  pub(crate) fn request_future_from<Resp, S>(&self, message: U, sender: &ActorRef<S, R>) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_future_with_dispatcher(message, sender.to_dispatcher())
  }

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
