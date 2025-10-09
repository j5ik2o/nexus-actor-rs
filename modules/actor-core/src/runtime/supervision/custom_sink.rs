use alloc::boxed::Box;
use core::marker::PhantomData;

use crate::FailureInfo;
use crate::{MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

type FailureHandler<M> = dyn FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static;

use crate::EscalationSink;

/// Sink based on custom handler.
pub(crate) struct CustomEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  handler: Box<FailureHandler<M>>,
  _marker: PhantomData<R>,
}

impl<M, R> CustomEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new<F>(handler: F) -> Self
  where
    F: FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static, {
    Self {
      handler: Box::new(handler),
      _marker: PhantomData,
    }
  }
}

impl<M, R> EscalationSink<M, R> for CustomEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, _already_handled: bool) -> Result<(), FailureInfo> {
    if (self.handler)(&info).is_ok() {
      Ok(())
    } else {
      Err(info)
    }
  }
}
