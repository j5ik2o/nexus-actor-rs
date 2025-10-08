use core::convert::Infallible;

use super::root_context::RootContext;
use crate::api::guardian::AlwaysRestart;
use crate::api::messaging::MessageEnvelope;
use crate::runtime::system::InternalActorSystem;
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

pub struct ActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: InternalActorSystem<MessageEnvelope<U>, R, Strat>,
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
    }
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
}
