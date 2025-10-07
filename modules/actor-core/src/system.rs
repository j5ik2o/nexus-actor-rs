use alloc::boxed::Box;
use alloc::sync::Arc;
use core::convert::Infallible;

use crate::context::{ActorContext, PriorityActorRef};
use crate::guardian::{AlwaysRestart, GuardianStrategy};
use crate::mailbox::SystemMessage;
use crate::scheduler::PriorityScheduler;
use crate::supervisor::{NoopSupervisor, Supervisor};
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

pub struct Props<M, R>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  pub options: MailboxOptions,
  pub map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  pub handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
}

impl<M, R> Props<M, R>
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

pub struct ActorSystem<M, R, Strat = AlwaysRestart>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  scheduler: PriorityScheduler<M, R, Strat>,
}

impl<M, R> ActorSystem<M, R, AlwaysRestart>
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

pub struct RootContext<'a, M, R, Strat>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  system: &'a mut ActorSystem<M, R, Strat>,
}

impl<'a, M, R, Strat> RootContext<'a, M, R, Strat>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn spawn(&mut self, props: Props<M, R>) -> Result<PriorityActorRef<M, R>, QueueError<PriorityEnvelope<M>>> {
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

impl<M, R, Strat> ActorSystem<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn root_context(&mut self) -> RootContext<'_, M, R, Strat> {
    RootContext { system: self }
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

#[cfg(test)]
mod tests {
  #![allow(deprecated)]
  use super::*;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use crate::mailbox::{MailboxOptions, SystemMessage};
  use alloc::rc::Rc;
  use alloc::sync::Arc;
  use alloc::vec::Vec;
  use core::cell::RefCell;
  use nexus_utils_core_rs::{Element, DEFAULT_PRIORITY};

  #[derive(Debug, Clone)]
  enum Message {
    User(u32),
    System,
  }

  impl Element for Message {}

  #[test]
  fn actor_system_spawns_and_processes_messages() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut system: ActorSystem<Message, _, AlwaysRestart> = ActorSystem::new(runtime);

    let map_system = Arc::new(|_: SystemMessage| Message::System);
    let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let mut root = system.root_context();
    let actor_ref = root
      .spawn(Props::new(
        MailboxOptions::default(),
        map_system.clone(),
        move |_, msg: Message| match msg {
          Message::User(value) => log_clone.borrow_mut().push(value),
          Message::System => {}
        },
      ))
      .expect("spawn actor");

    actor_ref
      .try_send_with_priority(Message::User(7), DEFAULT_PRIORITY)
      .expect("send message");

    root.dispatch_all().expect("dispatch");

    assert_eq!(log.borrow().as_slice(), &[7]);
  }
}
