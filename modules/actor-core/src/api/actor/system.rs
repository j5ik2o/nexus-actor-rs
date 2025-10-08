use alloc::sync::Arc;
use core::convert::Infallible;
use core::sync::atomic::{AtomicBool, Ordering};

use super::root_context::RootContext;
use super::{ActorSystemHandles, ActorSystemParts, Spawn, Timer};
use crate::api::guardian::AlwaysRestart;
use crate::api::messaging::MessageEnvelope;
use crate::runtime::system::InternalActorSystem;
use crate::{FailureEventListener, FailureEventStream, MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

pub struct ActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: InternalActorSystem<MessageEnvelope<U>, R, Strat>,
  shutdown: ShutdownToken,
}

pub struct ActorSystemRunner<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  system: ActorSystem<U, R, Strat>,
}

impl<U, R> ActorSystem<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new(mailbox_factory: R) -> Self {
    Self {
      inner: InternalActorSystem::new(mailbox_factory),
      shutdown: ShutdownToken::default(),
    }
  }

  pub fn set_failure_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.inner.set_root_event_listener(listener);
  }

  pub fn from_parts<S, T, E>(parts: ActorSystemParts<R, S, T, E>) -> (Self, ActorSystemHandles<S, T, E>)
  where
    S: Spawn,
    T: Timer,
    E: FailureEventStream, {
    let (mailbox_factory, handles) = parts.split();
    let mut system = Self::new(mailbox_factory);
    system.set_failure_event_listener(Some(handles.event_stream.listener()));
    (system, handles)
  }
}

impl<U, R, Strat> ActorSystem<U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  pub fn into_runner(self) -> ActorSystemRunner<U, R, Strat> {
    ActorSystemRunner { system: self }
  }

  pub fn root_context(&mut self) -> RootContext<'_, U, R, Strat> {
    RootContext {
      inner: self.inner.root_context(),
    }
  }

  pub async fn run_until<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>
  where
    F: FnMut() -> bool, {
    self.inner.run_until(should_continue).await
  }

  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.run_forever().await
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>
  where
    F: FnMut() -> bool, {
    self.inner.blocking_dispatch_loop(should_continue)
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.blocking_dispatch_forever()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.dispatch_next().await
  }

  /// Ready キューに溜まったメッセージを同期的に処理し、空になるまで繰り返す。
  /// 新たにメッセージが到着するまで待機は行わない。
  pub fn run_until_idle(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let shutdown = self.shutdown.clone();
    self.inner.run_until_idle(|| !shutdown.is_triggered())
  }
}

impl<U, R, Strat> ActorSystemRunner<U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.system.shutdown.clone()
  }

  pub async fn run_forever(mut self) -> Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.system.run_forever().await
  }

  pub async fn into_future(self) -> Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.run_forever().await
  }

  pub fn into_inner(self) -> ActorSystem<U, R, Strat> {
    self.system
  }
}

#[derive(Clone)]
pub struct ShutdownToken {
  inner: Arc<AtomicBool>,
}

impl ShutdownToken {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn trigger(&self) {
    self.inner.store(true, Ordering::SeqCst);
  }

  pub fn is_triggered(&self) -> bool {
    self.inner.load(Ordering::SeqCst)
  }
}

impl Default for ShutdownToken {
  fn default() -> Self {
    Self::new()
  }
}
