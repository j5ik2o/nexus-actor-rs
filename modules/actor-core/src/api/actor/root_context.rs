use crate::runtime::system::InternalRootContext;
use crate::{ActorRef, MailboxRuntime, MessageEnvelope, PriorityEnvelope, Props};
use nexus_utils_core_rs::{Element, QueueError};

pub struct RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  pub(crate) inner: InternalRootContext<'a, MessageEnvelope<U>, R, Strat>,
}

impl<'a, U, R, Strat> RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
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
}
