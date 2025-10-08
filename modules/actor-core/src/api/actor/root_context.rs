use crate::runtime::message::DynMessage;
use crate::runtime::system::InternalRootContext;
use crate::{ActorRef, MailboxFactory, PriorityEnvelope, Props};
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError};

pub struct RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  pub(crate) inner: InternalRootContext<'a, DynMessage, R, Strat>,
  pub(crate) _marker: PhantomData<U>,
}

impl<'a, U, R, Strat> RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  pub fn spawn(&mut self, props: Props<U, R>) -> Result<ActorRef<U, R>, QueueError<PriorityEnvelope<DynMessage>>> {
    let actor_ref = self.inner.spawn(props.into_inner())?;
    Ok(ActorRef::new(actor_ref))
  }

  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    #[allow(deprecated)]
    self.inner.dispatch_all()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.dispatch_next().await
  }
}
