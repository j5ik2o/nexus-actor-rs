use crate::context::InternalActorRef;
use crate::guardian::GuardianStrategy;
use crate::supervisor::NoopSupervisor;
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::{ActorSystem, Props};

pub struct RootContext<'a, M, R, Strat>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub(super) system: &'a mut ActorSystem<M, R, Strat>,
}

impl<'a, M, R, Strat> RootContext<'a, M, R, Strat>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn spawn(&mut self, props: Props<M, R>) -> Result<InternalActorRef<M, R>, QueueError<PriorityEnvelope<M>>> {
    let Props {
      options,
      map_system,
      mut handler,
    } = props;

    self
      .system
      .scheduler
      .spawn_actor(NoopSupervisor, options, map_system, move |ctx, msg| {
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
