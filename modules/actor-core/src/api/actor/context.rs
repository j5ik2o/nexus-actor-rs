use crate::runtime::context::ActorContext;
use crate::runtime::message::{DynMessage, MetadataStorageMode};
use crate::ActorId;
use crate::ActorPath;
use crate::Extension;
use crate::ExtensionId;
use crate::Extensions;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::RuntimeBound;
use crate::Supervisor;
use crate::SystemMessage;
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use alloc::{boxed::Box, string::String};
use core::future::Future;
use core::marker::PhantomData;
use core::time::Duration;
use nexus_utils_core_rs::sync::{ArcShared, SharedBound};

#[cfg(target_has_atomic = "ptr")]
type AdapterFn<Ext, U> = dyn Fn(Ext) -> U + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type AdapterFn<Ext, U> = dyn Fn(Ext) -> U;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::{
  ask::create_ask_handles, ask_with_timeout, ActorRef, AskError, AskFuture, AskResult, AskTimeoutFuture, Props,
};
use crate::api::{MessageEnvelope, MessageMetadata, MessageSender};

/// Typed actor execution context wrapper.
/// 'r: lifetime of the mutable reference to ActorContext
/// 'ctx: lifetime parameter of ActorContext itself
pub struct Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  R::Concurrency: MetadataStorageMode, {
  inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
  metadata: Option<MessageMetadata<R::Concurrency>>,
  extensions: Extensions,
  _marker: PhantomData<U>,
}

/// Type alias for context during setup.
pub type SetupContext<'ctx, U, R> = Context<'ctx, 'ctx, U, R>;

/// Context log level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextLogLevel {
  /// Trace level
  Trace,
  /// Debug level
  Debug,
  /// Info level
  Info,
  /// Warn level
  Warn,
  /// Error level
  Error,
}

/// Structure that manages actor log output.
#[derive(Clone)]
pub struct ContextLogger {
  actor_id: ActorId,
  actor_path: ActorPath,
}

impl ContextLogger {
  pub(crate) fn new(actor_id: ActorId, actor_path: &ActorPath) -> Self {
    Self {
      actor_id,
      actor_path: actor_path.clone(),
    }
  }

  /// Gets the actor ID of the log source.
  pub fn actor_id(&self) -> ActorId {
    self.actor_id
  }

  /// Gets the actor path of the log source.
  pub fn actor_path(&self) -> &ActorPath {
    &self.actor_path
  }

  /// Outputs a trace level log.
  pub fn trace<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Trace, message);
  }

  /// Outputs a debug level log.
  pub fn debug<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Debug, message);
  }

  /// Outputs an info level log.
  pub fn info<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Info, message);
  }

  /// Outputs a warn level log.
  pub fn warn<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Warn, message);
  }

  /// Outputs an error level log.
  pub fn error<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Error, message);
  }

  fn emit<F>(&self, level: ContextLogLevel, message: F)
  where
    F: FnOnce() -> String, {
    #[cfg(feature = "tracing")]
    {
      let text = message();
      match level {
        ContextLogLevel::Trace => tracing::event!(
          target: "nexus::actor",
          tracing::Level::TRACE,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Debug => tracing::event!(
          target: "nexus::actor",
          tracing::Level::DEBUG,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Info => tracing::event!(
          target: "nexus::actor",
          tracing::Level::INFO,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Warn => tracing::event!(
          target: "nexus::actor",
          tracing::Level::WARN,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Error => tracing::event!(
          target: "nexus::actor",
          tracing::Level::ERROR,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
      }
    }

    #[cfg(not(feature = "tracing"))]
    {
      let _ = level;
      let _ = message;
    }
  }
}

impl<'r, 'ctx, U, R> Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  R::Concurrency: MetadataStorageMode,
{
  pub(super) fn new(inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>) -> Self {
    let extensions = inner.extensions();
    Self {
      inner,
      metadata: None,
      extensions,
      _marker: PhantomData,
    }
  }

  /// Gets the metadata accompanying the current message.
  ///
  /// # Returns
  /// `Some(&MessageMetadata)` if metadata exists, `None` otherwise
  pub fn message_metadata(&self) -> Option<&MessageMetadata<R::Concurrency>> {
    self.metadata.as_ref()
  }

  pub(super) fn with_metadata(
    inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
    metadata: MessageMetadata<R::Concurrency>,
  ) -> Self {
    let extensions = inner.extensions();
    Self {
      inner,
      metadata: Some(metadata),
      extensions,
      _marker: PhantomData,
    }
  }

  /// Returns the shared extension registry.
  pub fn extensions(&self) -> Extensions {
    self.extensions.clone()
  }

  /// Applies the provided closure to the extension identified by `id`.
  pub fn extension<E, F, T>(&self, id: ExtensionId, f: F) -> Option<T>
  where
    E: Extension,
    F: FnOnce(&E) -> T, {
    self.extensions.with::<E, _, _>(id, f)
  }

  /// Gets the actor ID of this actor.
  ///
  /// # Returns
  /// Actor ID
  pub fn actor_id(&self) -> ActorId {
    self.inner.actor_id()
  }

  /// Gets the actor path of this actor.
  ///
  /// # Returns
  /// Reference to the actor path
  pub fn actor_path(&self) -> &ActorPath {
    self.inner.actor_path()
  }

  /// Gets the list of actor IDs watching this actor.
  ///
  /// # Returns
  /// Slice of watcher actor IDs
  pub fn watchers(&self) -> &[ActorId] {
    self.inner.watchers()
  }

  /// Gets the logger for this actor.
  ///
  /// # Returns
  /// Context logger
  pub fn log(&self) -> ContextLogger {
    ContextLogger::new(self.actor_id(), self.actor_path())
  }

  /// Sends a message to itself.
  ///
  /// # Arguments
  /// * `message` - Message to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn send_to_self(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(MessageEnvelope::user(message));
    self.inner.send_to_self_with_priority(dyn_message, DEFAULT_PRIORITY)
  }

  /// Sends a system message to itself.
  ///
  /// # Arguments
  /// * `message` - System message to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn send_system_to_self(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.send_envelope_to_self(envelope)
  }

  /// Gets a reference to itself.
  ///
  /// # Returns
  /// `ActorRef` to itself
  pub fn self_ref(&self) -> ActorRef<U, R> {
    ActorRef::new(self.inner.self_ref())
  }

  /// Creates an adapter that converts external message types to internal message types.
  ///
  /// # Arguments
  /// * `f` - Message conversion function
  ///
  /// # Returns
  /// Message adapter reference
  pub fn message_adapter<Ext, F>(&self, f: F) -> MessageAdapterRef<Ext, U, R>
  where
    Ext: Element,
    F: Fn(Ext) -> U + SharedBound + 'static, {
    let adapter_impl: Arc<AdapterFn<Ext, U>> = Arc::new(f);
    MessageAdapterRef::new(self.self_ref(), ArcShared::from_arc(adapter_impl))
  }

  /// Registers a watcher.
  ///
  /// # Arguments
  /// * `watcher` - Actor ID of the watcher
  pub fn register_watcher(&mut self, watcher: ActorId) {
    self.inner.register_watcher(watcher);
  }

  /// Unregisters a watcher.
  ///
  /// # Arguments
  /// * `watcher` - Actor ID of the watcher
  pub fn unregister_watcher(&mut self, watcher: ActorId) {
    self.inner.unregister_watcher(watcher);
  }

  /// Determines if receive timeout is supported.
  ///
  /// # Returns
  /// `true` if supported, `false` otherwise
  pub fn has_receive_timeout_support(&self) -> bool {
    self.inner.has_receive_timeout_scheduler()
  }

  /// Sets the receive timeout.
  ///
  /// # Arguments
  /// * `duration` - Timeout duration
  ///
  /// # Returns
  /// `true` on success, `false` otherwise
  pub fn set_receive_timeout(&mut self, duration: Duration) -> bool {
    self.inner.set_receive_timeout(duration)
  }

  /// Cancels the receive timeout.
  ///
  /// # Returns
  /// `true` on success, `false` otherwise
  pub fn cancel_receive_timeout(&mut self) -> bool {
    self.inner.cancel_receive_timeout()
  }

  /// Gets a mutable reference to the internal context.
  ///
  /// # Returns
  /// Mutable reference to the internal `ActorContext`
  pub fn inner(&mut self) -> &mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>> {
    self.inner
  }

  pub(crate) fn self_dispatcher(&self) -> MessageSender<U, R::Concurrency>
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    self.self_ref().to_dispatcher()
  }

  /// Requests a message with sender information.
  ///
  /// Sends a message with itself set as the sender.
  ///
  /// # Arguments
  /// * `target` - Target actor to send the message to
  /// * `message` - Message to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn request<V>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let metadata = MessageMetadata::<R::Concurrency>::new().with_sender(self.self_dispatcher());
    target.tell_with_metadata(message, metadata)
  }

  /// Requests a message with specified sender information.
  ///
  /// # Arguments
  /// * `target` - Target actor to send the message to
  /// * `message` - Message to send
  /// * `sender` - Actor to set as the sender
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn request_with_sender<V, S>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
    sender: &ActorRef<S, R>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element,
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let metadata = MessageMetadata::<R::Concurrency>::new().with_sender(sender.to_dispatcher());
    target.tell_with_metadata(message, metadata)
  }

  /// Forwards a message while preserving the original metadata.
  ///
  /// Forwards a message using the current message's metadata (sender information) as is.
  ///
  /// # Arguments
  /// * `target` - Target actor to forward the message to
  /// * `message` - Message to forward
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn forward<V>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element, {
    let metadata = self.message_metadata().cloned().unwrap_or_default();
    target.tell_with_metadata(message, metadata)
  }

  /// Responds to the sender of the current message.
  ///
  /// # Arguments
  /// * `message` - Response message
  ///
  /// # Returns
  /// `Ok(())` on success, `AskError` on failure
  ///
  /// # Errors
  /// - `AskError::MissingResponder` - If responder is not found
  /// - `AskError::SendFailed` - If message sending fails
  pub fn respond<Resp>(&mut self, message: Resp) -> AskResult<()>
  where
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let metadata = self.message_metadata().cloned().ok_or(AskError::MissingResponder)?;
    metadata.respond_with(self, message)
  }

  /// Sends an inquiry to the target actor and returns a Future that waits for a response.
  ///
  /// Constructs a message including a responder using a message factory.
  ///
  /// # Arguments
  /// * `target` - Target actor for the inquiry
  /// * `factory` - Function that generates a message using the responder
  ///
  /// # Returns
  /// `AskFuture` for receiving the response, or an error
  pub fn ask<V, Resp, F>(&mut self, target: &ActorRef<V, R>, factory: F) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element,
    F: FnOnce(MessageSender<Resp, R::Concurrency>) -> V,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let (future, responder) = create_ask_handles::<Resp, R::Concurrency>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::<R::Concurrency>::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    target.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// Sends an inquiry with timeout and returns a Future that waits for a response.
  ///
  /// # Arguments
  /// * `target` - Target actor for the inquiry
  /// * `factory` - Function that generates a message using the responder
  /// * `timeout` - Future for timeout control
  ///
  /// # Returns
  /// `AskTimeoutFuture` for receiving the response with timeout, or an error
  pub fn ask_with_timeout<V, Resp, F, TFut>(
    &mut self,
    target: &ActorRef<V, R>,
    factory: F,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    F: FnOnce(MessageSender<Resp, R::Concurrency>) -> V,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp, R::Concurrency>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::<R::Concurrency>::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    match target.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// Sends an inquiry to the target actor and returns a Future that waits for a response.
  ///
  /// # Arguments
  /// * `target` - Target actor for the inquiry
  /// * `message` - Message to send
  ///
  /// # Returns
  /// `AskFuture` for receiving the response, or an error
  pub fn request_future<V, Resp>(&mut self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let (future, responder) = create_ask_handles::<Resp, R::Concurrency>();
    let metadata = MessageMetadata::<R::Concurrency>::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    target.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// Sends an inquiry with timeout and returns a Future that waits for a response.
  ///
  /// # Arguments
  /// * `target` - Target actor for the inquiry
  /// * `message` - Message to send
  /// * `timeout` - Future for timeout control
  ///
  /// # Returns
  /// `AskTimeoutFuture` for receiving the response with timeout, or an error
  pub fn request_future_with_timeout<V, Resp, TFut>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp, R::Concurrency>();
    let metadata = MessageMetadata::<R::Concurrency>::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    match target.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// Spawns a child actor and returns an `ActorRef`.
  pub fn spawn_child<V>(&mut self, props: Props<V, R>) -> ActorRef<V, R>
  where
    V: Element, {
    let (internal_props, supervisor_cfg) = props.into_parts();
    let actor_ref = self
      .inner
      .spawn_child_from_props(Box::new(supervisor_cfg.into_supervisor()), internal_props);
    ActorRef::new(actor_ref)
  }
}

impl<C> MessageMetadata<C>
where
  C: MetadataStorageMode,
{
  /// Sends a response message using metadata.
  ///
  /// # Arguments
  /// * `ctx` - Current context
  /// * `message` - Response message to send
  ///
  /// # Returns
  /// `Ok(())` on success, `AskError` on failure
  ///
  /// # Errors
  /// - `AskError::MissingResponder` - If responder is not found
  /// - `AskError::SendFailed` - If message sending fails
  pub fn respond_with<Resp, U, R>(&self, ctx: &mut Context<'_, '_, U, R>, message: Resp) -> AskResult<()>
  where
    Resp: Element,
    U: Element,
    R: MailboxFactory<Concurrency = C> + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + RuntimeBound + 'static,
    R::Signal: Clone + RuntimeBound + 'static, {
    let dispatcher = self.dispatcher_for::<Resp>().ok_or(AskError::MissingResponder)?;
    let dispatch_metadata = MessageMetadata::<C>::new().with_sender(ctx.self_dispatcher());
    let envelope = MessageEnvelope::user_with_metadata(message, dispatch_metadata);
    dispatcher.dispatch_envelope(envelope).map_err(AskError::from)
  }
}

/// Reference to a message adapter.
///
/// Converts external message types to internal message types and sends them to the target actor.
#[derive(Clone)]
pub struct MessageAdapterRef<Ext, U, R>
where
  Ext: Element,
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  target: ActorRef<U, R>,
  adapter: ArcShared<AdapterFn<Ext, U>>,
}

impl<Ext, U, R> MessageAdapterRef<Ext, U, R>
where
  Ext: Element,
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new(target: ActorRef<U, R>, adapter: ArcShared<AdapterFn<Ext, U>>) -> Self {
    Self { target, adapter }
  }

  /// Converts an external message and sends it to the target actor.
  ///
  /// # Arguments
  /// * `message` - External message to send
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn tell(&self, message: Ext) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let mapped = (self.adapter)(message);
    self.target.tell(mapped)
  }

  /// Converts an external message and sends it to the target actor with the specified priority.
  ///
  /// # Arguments
  /// * `message` - External message to send
  /// * `priority` - Message priority
  ///
  /// # Returns
  /// `Ok(())` on success, queue error on failure
  pub fn tell_with_priority(&self, message: Ext, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let mapped = (self.adapter)(message);
    self.target.tell_with_priority(mapped, priority)
  }

  /// Gets a reference to the target actor.
  ///
  /// # Returns
  /// Reference to the target `ActorRef`
  pub fn target(&self) -> &ActorRef<U, R> {
    &self.target
  }
}
