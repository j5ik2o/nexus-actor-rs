use core::convert::Infallible;

use crate::runtime::guardian::{AlwaysRestart, GuardianStrategy};
use crate::runtime::scheduler::PriorityScheduler;
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::InternalRootContext;

pub(crate) struct InternalActorSystem<M, R, Strat = AlwaysRestart>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  pub(super) scheduler: PriorityScheduler<M, R, Strat>,
}

impl<M, R> InternalActorSystem<M, R, AlwaysRestart>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      scheduler: PriorityScheduler::new(runtime),
    }
  }
}

impl<M, R, Strat> InternalActorSystem<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn root_context(&mut self) -> InternalRootContext<'_, M, R, Strat> {
    InternalRootContext { system: self }
  }

  pub async fn run_until<F>(&mut self, should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool, {
    self.scheduler.run_until(should_continue).await
  }

  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<M>>> {
    self.scheduler.run_forever().await
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(&mut self, should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool, {
    self.scheduler.blocking_dispatch_loop(should_continue)
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<M>>> {
    self.scheduler.blocking_dispatch_forever()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.scheduler.dispatch_next().await
  }
}
