use crate::runtime::message::DynMessage;
use crate::runtime::system::InternalRootContext;
use crate::{ActorRef, MailboxFactory, PriorityEnvelope, Props};
use alloc::boxed::Box;
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError};

use super::{ask_with_timeout, AskFuture, AskResult, AskTimeoutFuture};

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
    let (internal_props, supervisor_cfg) = props.into_parts();
    let actor_ref = self
      .inner
      .spawn_with_supervisor(Box::new(supervisor_cfg.into_supervisor()), internal_props)?;
    Ok(ActorRef::new(actor_ref))
  }

  pub fn request_future<V, Resp>(&self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element, {
    target.request_future(message)
  }

  pub fn request_future_with_timeout<V, Resp, TFut>(
    &self,
    target: &ActorRef<V, R>,
    message: V,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    TFut: Future<Output = ()> + Unpin, {
    let future = target.request_future(message)?;
    Ok(ask_with_timeout(future, timeout))
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
