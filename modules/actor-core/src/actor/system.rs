use core::convert::Infallible;

use crate::guardian::AlwaysRestart;
use crate::system::{ActorSystem as InternalActorSystem, RootContext as InternalRootContext};
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::{ActorRef, MessageEnvelope, Props};

pub struct ActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
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
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn root_context(&mut self) -> RootContext<'_, U, R, Strat> {
    RootContext {
      inner: self.inner.root_context(),
    }
  }

  pub fn inner(&mut self) -> &mut InternalActorSystem<MessageEnvelope<U>, R, Strat> {
    &mut self.inner
  }

  pub async fn run_until<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>
  where
    F: FnMut() -> bool,
  {
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
    F: FnMut() -> bool,
  {
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

pub struct RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  inner: InternalRootContext<'a, MessageEnvelope<U>, R, Strat>,
}

impl<'a, U, R, Strat> RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn spawn(
    &mut self,
    props: Props<U, R>,
  ) -> Result<ActorRef<U, R>, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let actor_ref = self.inner.spawn(props.into_inner())?;
    Ok(ActorRef::new(actor_ref))
  }

  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    #[allow(deprecated)]
    self.inner.dispatch_all()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.dispatch_next().await
  }

  pub fn raw(&mut self) -> &mut InternalRootContext<'a, MessageEnvelope<U>, R, Strat> {
    &mut self.inner
  }
}
