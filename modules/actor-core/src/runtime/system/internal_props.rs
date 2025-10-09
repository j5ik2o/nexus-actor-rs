use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::runtime::context::{ActorContext, ActorHandlerFn, MapSystemFn};
use crate::Supervisor;
use crate::{MailboxFactory, MailboxOptions, PriorityEnvelope};
use nexus_utils_core_rs::Element;

pub(crate) struct InternalProps<M, R>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub options: MailboxOptions,
  pub map_system: Arc<MapSystemFn<M>>,
  pub handler: Box<ActorHandlerFn<M, R>>,
}

impl<M, R> InternalProps<M, R>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(
    options: MailboxOptions,
    map_system: Arc<MapSystemFn<M>>,
    handler: impl for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
  ) -> Self {
    Self {
      options,
      map_system,
      handler: Box::new(handler),
    }
  }
}
