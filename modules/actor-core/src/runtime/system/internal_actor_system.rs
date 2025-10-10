use core::convert::Infallible;

use crate::runtime::guardian::{AlwaysRestart, GuardianStrategy};
use crate::runtime::scheduler::PriorityScheduler;
use crate::ReceiveTimeoutFactoryShared;
use crate::{FailureEventListener, MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::InternalRootContext;

/// Internal configuration used while assembling [`InternalActorSystem`].
pub struct InternalActorSystemSettings<M, R>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// Listener invoked for failures reaching the root guardian.
  pub root_event_listener: Option<FailureEventListener>,
  /// Receive-timeout scheduler factory applied to newly spawned actors.
  pub receive_timeout_factory: Option<ReceiveTimeoutFactoryShared<M, R>>,
}

impl<M, R> Default for InternalActorSystemSettings<M, R>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn default() -> Self {
    Self {
      root_event_listener: None,
      receive_timeout_factory: None,
    }
  }
}

pub(crate) struct InternalActorSystem<M, R, Strat = AlwaysRestart>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  pub(super) scheduler: PriorityScheduler<M, R, Strat>,
}

#[allow(dead_code)]
impl<M, R> InternalActorSystem<M, R, AlwaysRestart>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(mailbox_factory: R) -> Self {
    Self::new_with_settings(mailbox_factory, InternalActorSystemSettings::default())
  }

  pub fn new_with_settings(mailbox_factory: R, settings: InternalActorSystemSettings<M, R>) -> Self {
    let mut scheduler = PriorityScheduler::new(mailbox_factory);
    let InternalActorSystemSettings {
      root_event_listener,
      receive_timeout_factory,
    } = settings;
    scheduler.set_root_event_listener(root_event_listener);
    scheduler.set_receive_timeout_factory(receive_timeout_factory);
    Self { scheduler }
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

  pub fn drain_ready(&mut self) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    self.scheduler.drain_ready()
  }

  pub fn run_until_idle<F>(&mut self, mut should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool, {
    while should_continue() {
      let processed = self.drain_ready()?;
      if !processed {
        break;
      }
    }
    Ok(())
  }
}
