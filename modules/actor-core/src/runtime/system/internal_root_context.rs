use alloc::boxed::Box;

use crate::runtime::context::InternalActorRef;
use crate::runtime::guardian::GuardianStrategy;
use crate::NoopSupervisor;
use crate::{MailboxFactory, PriorityEnvelope, Supervisor};
use nexus_utils_core_rs::{Element, QueueError};

use super::{InternalActorSystem, InternalProps};

pub(crate) struct InternalRootContext<'a, M, R, Strat>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub(super) system: &'a mut InternalActorSystem<M, R, Strat>,
}

impl<'a, M, R, Strat> InternalRootContext<'a, M, R, Strat>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  #[allow(dead_code)]
  pub fn spawn(
    &mut self,
    props: InternalProps<M, R>,
  ) -> Result<InternalActorRef<M, R>, QueueError<PriorityEnvelope<M>>> {
    self.spawn_with_supervisor(Box::new(NoopSupervisor), props)
  }

  pub fn spawn_with_supervisor(
    &mut self,
    supervisor: Box<dyn Supervisor<M>>,
    props: InternalProps<M, R>,
  ) -> Result<InternalActorRef<M, R>, QueueError<PriorityEnvelope<M>>> {
    let InternalProps {
      options,
      map_system,
      mut handler,
    } = props;

    self
      .system
      .scheduler
      .spawn_actor(supervisor, options, map_system, move |ctx, msg| {
        handler(ctx, msg);
      })
  }

  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    #[allow(deprecated)]
    self.system.scheduler.dispatch_all()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.system.scheduler.dispatch_next().await
  }
}
