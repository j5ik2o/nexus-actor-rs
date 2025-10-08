use core::convert::Infallible;

use crate::guardian::AlwaysRestart;
use crate::system::{ActorSystem, RootContext};
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::{MessageEnvelope, TypedActorRef, TypedProps};

pub struct TypedActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: ActorSystem<MessageEnvelope<U>, R, Strat>,
}

impl<U, R> TypedActorSystem<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      inner: ActorSystem::new(runtime),
    }
  }
}

impl<U, R, Strat> TypedActorSystem<U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn root_context(&mut self) -> TypedRootContext<'_, U, R, Strat> {
    TypedRootContext {
      inner: self.inner.root_context(),
    }
  }

  pub fn inner(&mut self) -> &mut ActorSystem<MessageEnvelope<U>, R, Strat> {
    &mut self.inner
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

pub struct TypedRootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: RootContext<'a, MessageEnvelope<U>, R, Strat>,
}

impl<'a, U, R, Strat> TypedRootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn spawn(
    &mut self,
    props: TypedProps<U, R>,
  ) -> Result<TypedActorRef<U, R>, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let actor_ref = self.inner.spawn(props.into_inner())?;
    Ok(TypedActorRef::new(actor_ref))
  }

  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    #[allow(deprecated)]
    self.inner.dispatch_all()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.dispatch_next().await
  }

  pub fn raw(&mut self) -> &mut RootContext<'a, MessageEnvelope<U>, R, Strat> {
    &mut self.inner
  }
}
