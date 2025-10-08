use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::runtime::context::ActorContext;
use crate::Supervisor;
use crate::SystemMessage;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::Element;

pub(crate) struct InternalProps<M, R>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  pub options: MailboxOptions,
  pub map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  pub handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
}

impl<M, R> InternalProps<M, R>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(
    options: MailboxOptions,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
    handler: impl for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
  ) -> Self {
    Self {
      options,
      map_system,
      handler: Box::new(handler),
    }
  }
}
