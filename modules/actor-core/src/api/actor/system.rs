use alloc::sync::Arc;
use core::convert::Infallible;
use core::future::Future;
use core::sync::atomic::{AtomicBool, Ordering};

use super::root_context::RootContext;
use crate::api::guardian::AlwaysRestart;
use crate::api::messaging::MessageEnvelope;
use crate::api::runtime::{RuntimeComponentHandles, RuntimeComponents, Spawn, Timer};
use crate::runtime::system::InternalActorSystem;
use crate::{FailureEventListener, FailureEventStream, MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

pub struct ActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: InternalActorSystem<MessageEnvelope<U>, R, Strat>,
  shutdown: ShutdownToken,
}

pub struct ActorSystemRunner<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  system: ActorSystem<U, R, Strat>,
}

impl<U, R> ActorSystem<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      inner: InternalActorSystem::new(runtime),
      shutdown: ShutdownToken::default(),
    }
  }

  pub fn set_failure_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.inner.set_root_event_listener(listener);
  }

  pub fn from_runtime_components<S, T, E>(
    components: RuntimeComponents<R, S, T, E>,
  ) -> (Self, RuntimeComponentHandles<S, T, E>)
  where
    S: Spawn,
    T: Timer,
    E: FailureEventStream, {
    let (runtime, handles) = components.split();
    let mut system = Self::new(runtime);
    system.set_failure_event_listener(Some(handles.event_stream.listener()));
    (system, handles)
  }

}

impl<U, R, Strat> ActorSystem<U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
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
    loop {
      if self.shutdown.is_triggered() {
        break;
      }
      let processed = self.inner.drain_ready()?;
      if !processed {
        break;
      }
    }
    Ok(())
  }
}

impl<U, R, Strat> ActorSystemRunner<U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
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

  pub fn into_future(
    self,
  ) -> impl Future<Output = Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>>> {
    async move { self.run_forever().await }
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
    Self { inner: Arc::new(AtomicBool::new(false)) }
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
