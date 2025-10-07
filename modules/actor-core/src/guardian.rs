use alloc::collections::BTreeMap;
use core::fmt;

use crate::context::PriorityActorRef;
use crate::mailbox::{PriorityEnvelope, SystemMessage};
use crate::supervisor::SupervisorDirective;
use crate::MailboxRuntime;
use nexus_utils_core_rs::{Element, QueueError};

/// 子アクター識別子。protoactor-go の ProcessId に相当する簡易 ID。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(pub usize);

/// Supervisor 戦略。protoactor-go の Strategy に相当する。
pub trait GuardianStrategy<M, R>: Send + 'static
where
  M: Element,
  R: MailboxRuntime, {
  fn decide(&mut self, actor: ActorId, error: &dyn fmt::Debug) -> SupervisorDirective;
  fn before_start(&mut self, _actor: ActorId) {}
  fn after_restart(&mut self, _actor: ActorId) {}
}

/// 最も単純な戦略: 常に Restart を指示する。
#[derive(Clone, Copy, Debug, Default)]
pub struct AlwaysRestart;

impl<M, R> GuardianStrategy<M, R> for AlwaysRestart
where
  M: Element,
  R: MailboxRuntime,
{
  fn decide(&mut self, _actor: ActorId, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Restart
  }
}

struct ChildRecord<R>
where
  R: MailboxRuntime, {
  control_ref: PriorityActorRef<SystemMessage, R>,
}

/// Guardian: 子アクター群を監督し、SystemMessage を送出する。
pub struct Guardian<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime,
  Strat: GuardianStrategy<M, R>, {
  next_id: usize,
  children: BTreeMap<ActorId, ChildRecord<R>>,
  strategy: Strat,
  _marker: core::marker::PhantomData<M>,
}

impl<M, R, Strat> Guardian<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<SystemMessage>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn new(strategy: Strat) -> Self {
    Self {
      next_id: 0,
      children: BTreeMap::new(),
      strategy,
      _marker: core::marker::PhantomData,
    }
  }

  pub fn register_child(&mut self, control_ref: PriorityActorRef<SystemMessage, R>) -> ActorId {
    let id = ActorId(self.next_id);
    self.next_id += 1;
    self.strategy.before_start(id);
    self.children.insert(id, ChildRecord { control_ref });
    id
  }

  pub fn remove_child(&mut self, id: ActorId) -> Option<PriorityActorRef<SystemMessage, R>> {
    self.children.remove(&id).map(|record| record.control_ref)
  }

  pub fn child_ref(&self, id: ActorId) -> Option<&PriorityActorRef<SystemMessage, R>> {
    self.children.get(&id).map(|record| &record.control_ref)
  }

  pub fn notify_failure(
    &mut self,
    actor: ActorId,
    error: &dyn fmt::Debug,
  ) -> Result<(), QueueError<PriorityEnvelope<SystemMessage>>> {
    let directive = self.strategy.decide(actor, error);
    match (directive, self.children.get(&actor)) {
      (SupervisorDirective::Resume, _) => Ok(()),
      (SupervisorDirective::Stop, Some(record)) => record.control_ref.try_send_system(SystemMessage::Stop),
      (SupervisorDirective::Restart, Some(record)) => {
        record.control_ref.try_send_system(SystemMessage::Restart)?;
        self.strategy.after_restart(actor);
        Ok(())
      }
      (SupervisorDirective::Escalate, _) => Ok(()),
      (_, None) => Ok(()),
    }
  }

  pub fn stop_child(&mut self, actor: ActorId) -> Result<(), QueueError<PriorityEnvelope<SystemMessage>>> {
    if let Some(record) = self.children.get(&actor) {
      record.control_ref.try_send_system(SystemMessage::Stop)
    } else {
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use crate::mailbox::{PriorityChannel, PriorityEnvelope};
  use nexus_utils_core_rs::DEFAULT_PRIORITY;

  #[test]
  fn guardian_sends_restart_message() {
    let runtime = TestMailboxRuntime::unbounded();
    let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
    let ref_control: PriorityActorRef<SystemMessage, TestMailboxRuntime> = PriorityActorRef::new(sender);

    let mut guardian: Guardian<SystemMessage, _, AlwaysRestart> = Guardian::new(AlwaysRestart);
    let actor_id = guardian.register_child(ref_control.clone());

    guardian.notify_failure(actor_id, &"panic").unwrap();

    let envelope = mailbox.queue().poll().unwrap().unwrap();
    let (message, priority, channel) = envelope.into_parts_with_channel();
    assert_eq!(message, SystemMessage::Restart);
    assert!(priority > DEFAULT_PRIORITY);
    assert_eq!(channel, PriorityChannel::Control);
  }

  #[test]
  fn guardian_sends_stop_message() {
    struct AlwaysStop;
    impl<M, R> GuardianStrategy<M, R> for AlwaysStop
    where
      M: Element,
      R: MailboxRuntime,
    {
      fn decide(&mut self, _actor: ActorId, _error: &dyn fmt::Debug) -> SupervisorDirective {
        SupervisorDirective::Stop
      }
    }

    let runtime = TestMailboxRuntime::unbounded();
    let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
    let ref_control: PriorityActorRef<SystemMessage, TestMailboxRuntime> = PriorityActorRef::new(sender);

    let mut guardian: Guardian<SystemMessage, _, AlwaysStop> = Guardian::new(AlwaysStop);
    let actor_id = guardian.register_child(ref_control.clone());

    guardian.notify_failure(actor_id, &"panic").unwrap();

    let envelope = mailbox.queue().poll().unwrap().unwrap();
    assert_eq!(envelope.into_parts().0, SystemMessage::Stop);
  }
}
