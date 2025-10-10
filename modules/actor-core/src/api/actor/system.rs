#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use core::convert::Infallible;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

use super::root_context::RootContext;
use super::{ActorSystemHandles, ActorSystemParts, Spawn, Timer};
use crate::api::guardian::AlwaysRestart;
use crate::runtime::message::DynMessage;
use crate::runtime::system::{InternalActorSystem, InternalActorSystemSettings};
use crate::serializer_extension_id;
use crate::ReceiveTimeoutFactoryShared;
use crate::{
  Extension, ExtensionId, Extensions, FailureEventListener, FailureEventStream, MailboxFactory, PriorityEnvelope,
  SerializerRegistryExtension,
};
use nexus_utils_core_rs::sync::ArcShared;
use nexus_utils_core_rs::{Element, QueueError};

/// Primary instance of the actor system.
///
/// Responsible for actor spawning, management, and message dispatching.
pub struct ActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  inner: InternalActorSystem<DynMessage, R, Strat>,
  shutdown: ShutdownToken,
  extensions: Extensions,
  _marker: PhantomData<U>,
}

/// Configuration options applied when constructing an [`ActorSystem`].
pub struct ActorSystemConfig<R>
where
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  /// Listener invoked when failures bubble up to the root guardian.
  failure_event_listener: Option<FailureEventListener>,
  /// Receive-timeout scheduler factory used by all actors spawned in the system.
  receive_timeout_factory: Option<ReceiveTimeoutFactoryShared<DynMessage, R>>,
  /// Extension registry configured for the actor system.
  extensions: Extensions,
}

impl<R> Default for ActorSystemConfig<R>
where
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  fn default() -> Self {
    Self {
      failure_event_listener: None,
      receive_timeout_factory: None,
      extensions: Extensions::new(),
    }
  }
}

impl<R> ActorSystemConfig<R>
where
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// Sets the failure event listener.
  pub fn with_failure_event_listener(mut self, listener: Option<FailureEventListener>) -> Self {
    self.failure_event_listener = listener;
    self
  }

  /// Sets the receive-timeout factory.
  pub fn with_receive_timeout_factory(mut self, factory: Option<ReceiveTimeoutFactoryShared<DynMessage, R>>) -> Self {
    self.receive_timeout_factory = factory;
    self
  }

  /// Mutable setter for the failure event listener.
  pub fn set_failure_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.failure_event_listener = listener;
  }

  /// Mutable setter for the receive-timeout factory.
  pub fn set_receive_timeout_factory(&mut self, factory: Option<ReceiveTimeoutFactoryShared<DynMessage, R>>) {
    self.receive_timeout_factory = factory;
  }

  pub(crate) fn failure_event_listener(&self) -> Option<FailureEventListener> {
    self.failure_event_listener.clone()
  }

  pub(crate) fn receive_timeout_factory(&self) -> Option<ReceiveTimeoutFactoryShared<DynMessage, R>> {
    self.receive_timeout_factory.clone()
  }

  /// Replaces the extension registry in the configuration.
  pub fn with_extensions(mut self, extensions: Extensions) -> Self {
    self.extensions = extensions;
    self
  }

  /// Registers an extension handle in the configuration.
  pub fn with_extension_handle<E>(self, extension: ArcShared<E>) -> Self
  where
    E: Extension, {
    let extensions = &self.extensions;
    extensions.register(extension);
    self
  }

  /// Registers an extension value in the configuration by wrapping it with `ArcShared`.
  pub fn with_extension_value<E>(self, extension: E) -> Self
  where
    E: Extension, {
    self.with_extension_handle(ArcShared::new(extension))
  }

  /// Returns the registered extensions.
  pub fn extensions(&self) -> Extensions {
    self.extensions.clone()
  }

  /// Mutably overrides the extensions registry.
  pub fn set_extensions(&mut self, extensions: Extensions) {
    self.extensions = extensions;
  }

  /// Registers an extension on the existing registry.
  pub fn register_extension<E>(&self, extension: ArcShared<E>)
  where
    E: Extension, {
    self.extensions.register(extension);
  }

  /// Registers a dynamically typed extension on the existing registry.
  pub fn register_extension_dyn(&self, extension: ArcShared<dyn Extension>) {
    self.extensions.register_dyn(extension);
  }
}

/// Execution runner for the actor system.
///
/// Wraps `ActorSystem` and provides an interface for execution on an asynchronous runtime.
pub struct ActorSystemRunner<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  system: ActorSystem<U, R, Strat>,
  _marker: PhantomData<U>,
}

impl<U, R> ActorSystem<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// Creates a new actor system with the specified mailbox factory.
  ///
  /// # Arguments
  /// * `mailbox_factory` - Factory that generates mailboxes
  pub fn new(mailbox_factory: R) -> Self {
    Self::new_with_config(mailbox_factory, ActorSystemConfig::default())
  }

  /// Creates a new actor system with an explicit configuration.
  pub fn new_with_config(mailbox_factory: R, config: ActorSystemConfig<R>) -> Self {
    let extensions_handle = config.extensions();
    if extensions_handle.get(serializer_extension_id()).is_none() {
      let extension = ArcShared::new(SerializerRegistryExtension::new());
      extensions_handle.register(extension);
    }
    let extensions = extensions_handle.clone();
    let settings = InternalActorSystemSettings {
      root_event_listener: config.failure_event_listener(),
      receive_timeout_factory: config.receive_timeout_factory(),
      extensions: extensions.clone(),
    };
    Self {
      inner: InternalActorSystem::new_with_settings(mailbox_factory, settings),
      shutdown: ShutdownToken::default(),
      extensions,
      _marker: PhantomData,
    }
  }

  /// Constructs an actor system and handles from parts.
  ///
  /// # Arguments
  /// * `parts` - Actor system parts
  ///
  /// # Returns
  /// Tuple of `(ActorSystem, ActorSystemHandles)`
  pub fn from_parts<S, T, E>(parts: ActorSystemParts<R, S, T, E>) -> (Self, ActorSystemHandles<S, T, E>)
  where
    S: Spawn,
    T: Timer,
    E: FailureEventStream, {
    let (mailbox_factory, handles) = parts.split();
    let config = ActorSystemConfig::default().with_failure_event_listener(Some(handles.event_stream.listener()));
    (Self::new_with_config(mailbox_factory, config), handles)
  }
}

impl<U, R, Strat> ActorSystem<U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  /// Gets the shutdown token.
  ///
  /// # Returns
  /// Clone of the shutdown token
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  /// Converts this system into a runner.
  ///
  /// The runner provides an interface suitable for execution on an asynchronous runtime.
  ///
  /// # Returns
  /// Actor system runner
  pub fn into_runner(self) -> ActorSystemRunner<U, R, Strat> {
    ActorSystemRunner {
      system: self,
      _marker: PhantomData,
    }
  }

  /// Gets the root context.
  ///
  /// The root context is used to spawn actors at the top level of the actor system.
  ///
  /// # Returns
  /// Mutable reference to the root context
  pub fn root_context(&mut self) -> RootContext<'_, U, R, Strat> {
    RootContext {
      inner: self.inner.root_context(),
      _marker: PhantomData,
    }
  }

  /// Returns a clone of the shared extension registry.
  pub fn extensions(&self) -> Extensions {
    self.extensions.clone()
  }

  /// Applies a closure to the specified extension and returns its result.
  pub fn extension<E, F, T>(&self, id: ExtensionId, f: F) -> Option<T>
  where
    E: Extension,
    F: FnOnce(&E) -> T, {
    self.extensions.with::<E, _, _>(id, f)
  }

  /// Registers an extension handle with the running actor system.
  pub fn register_extension<E>(&self, extension: ArcShared<E>)
  where
    E: Extension, {
    self.extensions.register(extension);
  }

  /// Registers a dynamically typed extension handle with the running actor system.
  pub fn register_extension_dyn(&self, extension: ArcShared<dyn Extension>) {
    self.extensions.register_dyn(extension);
  }

  /// Registers an extension value by wrapping it with `ArcShared`.
  pub fn register_extension_value<E>(&self, extension: E)
  where
    E: Extension, {
    self.register_extension(ArcShared::new(extension));
  }

  /// Executes message dispatching until the specified condition is met.
  ///
  /// # Arguments
  /// * `should_continue` - Closure that determines continuation condition. Continues execution while it returns `true`
  ///
  /// # Returns
  /// `Ok(())` on normal completion, `Err` on queue error
  pub async fn run_until<F>(&mut self, should_continue: F) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    F: FnMut() -> bool, {
    self.inner.run_until(should_continue).await
  }

  /// Executes message dispatching permanently.
  ///
  /// This function does not terminate normally. Returns only on error.
  ///
  /// # Returns
  /// `Infallible` (does not terminate normally) or queue error
  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.run_forever().await
  }

  /// Executes message dispatching in blocking mode until the specified condition is met.
  ///
  /// This function is only available when the standard library is enabled.
  ///
  /// # Arguments
  /// * `should_continue` - Closure that determines continuation condition. Continues execution while it returns `true`
  ///
  /// # Returns
  /// `Ok(())` on normal completion, `Err` on queue error
  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    F: FnMut() -> bool, {
    self.inner.blocking_dispatch_loop(should_continue)
  }

  /// Executes message dispatching permanently in blocking mode.
  ///
  /// This function is only available when the standard library is enabled. Does not terminate normally.
  ///
  /// # Returns
  /// `Infallible` (does not terminate normally) or queue error
  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.blocking_dispatch_forever()
  }

  /// Dispatches one next message.
  ///
  /// Waits until a new message arrives if the queue is empty.
  ///
  /// # Returns
  /// `Ok(())` on normal completion, `Err` on queue error
  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.dispatch_next().await
  }

  /// Synchronously processes messages accumulated in the Ready queue, repeating until empty.
  /// Does not wait for new messages to arrive.
  pub fn run_until_idle(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let shutdown = self.shutdown.clone();
    self.inner.run_until_idle(|| !shutdown.is_triggered())
  }
}

impl<U, R, Strat> ActorSystemRunner<U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  /// Gets the shutdown token.
  ///
  /// # Returns
  /// Clone of the shutdown token
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.system.shutdown.clone()
  }

  /// Executes message dispatching permanently.
  ///
  /// This function does not terminate normally. Returns only on error.
  ///
  /// # Returns
  /// `Infallible` (does not terminate normally) or queue error
  pub async fn run_forever(mut self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.system.run_forever().await
  }

  /// Executes the runner as a Future.
  ///
  /// Alias for `run_forever`. Provides a name suitable for execution on an asynchronous runtime.
  ///
  /// # Returns
  /// `Infallible` (does not terminate normally) or queue error
  pub async fn into_future(self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.run_forever().await
  }

  /// Extracts the internal actor system from the runner.
  ///
  /// # Returns
  /// Internal actor system
  pub fn into_inner(self) -> ActorSystem<U, R, Strat> {
    self.system
  }
}

/// Token that controls shutdown of the actor system.
///
/// Can be shared among multiple threads or tasks and cooperatively manages shutdown state.
#[derive(Clone)]
pub struct ShutdownToken {
  inner: Arc<AtomicBool>,
}

impl ShutdownToken {
  /// Creates a new shutdown token.
  ///
  /// Shutdown is not triggered in the initial state.
  ///
  /// # Returns
  /// New shutdown token
  pub fn new() -> Self {
    Self {
      inner: Arc::new(AtomicBool::new(false)),
    }
  }

  /// Triggers shutdown.
  ///
  /// This operation can be safely called from multiple threads.
  /// Once triggered, the state cannot be reset.
  pub fn trigger(&self) {
    self.inner.store(true, Ordering::SeqCst);
  }

  /// Checks whether shutdown has been triggered.
  ///
  /// # Returns
  /// `true` if shutdown has been triggered, `false` otherwise
  pub fn is_triggered(&self) -> bool {
    self.inner.load(Ordering::SeqCst)
  }
}

impl Default for ShutdownToken {
  fn default() -> Self {
    Self::new()
  }
}
