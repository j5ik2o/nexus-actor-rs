use core::convert::Infallible;

use alloc::sync::Arc;

use crate::runtime::guardian::{AlwaysRestart, GuardianStrategy};
use crate::runtime::scheduler::{PriorityScheduler, ReceiveTimeoutSchedulerFactory};
use crate::{FailureEventListener, MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::InternalRootContext;

pub(crate) struct InternalActorSystem<M, R, Strat = AlwaysRestart>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub(super) scheduler: PriorityScheduler<M, R, Strat>,
}

impl<M, R> InternalActorSystem<M, R, AlwaysRestart>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(mailbox_factory: R) -> Self {
    Self {
      scheduler: PriorityScheduler::new(mailbox_factory),
    }
  }
}

impl<M, R, Strat> InternalActorSystem<M, R, Strat>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn root_context(&mut self) -> InternalRootContext<'_, M, R, Strat> {
    InternalRootContext { system: self }
  }

  pub async fn run_until<F>(&mut self, should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool,
  {
    self.scheduler.run_until(should_continue).await
  }

  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<M>>> {
    self.scheduler.run_forever().await
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(&mut self, should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool,
  {
    self.scheduler.blocking_dispatch_loop(should_continue)
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<M>>> {
    self.scheduler.blocking_dispatch_forever()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.scheduler.dispatch_next().await
  }

  pub fn set_root_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.scheduler.set_root_event_listener(listener);
  }

  pub fn set_receive_timeout_factory(&mut self, factory: Option<Arc<dyn ReceiveTimeoutSchedulerFactory<M, R>>>) {
    self.scheduler.set_receive_timeout_factory(factory);
  }

  pub fn drain_ready(&mut self) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    self.scheduler.drain_ready()
  }

  pub fn run_until_idle<F>(&mut self, mut should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool,
  {
    while should_continue() {
      let processed = self.drain_ready()?;
      if !processed {
        break;
      }
    }
    Ok(())
  }
}
