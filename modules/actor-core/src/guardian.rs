use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::fmt;

use crate::actor_id::ActorId;
use crate::context::PriorityActorRef;
use crate::failure::FailureInfo;
use crate::mailbox::{PriorityEnvelope, SystemMessage};
use crate::supervisor::SupervisorDirective;
use crate::MailboxRuntime;
use nexus_utils_core_rs::{Element, QueueError};

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

struct ChildRecord<M, R>
where
  M: Element,
  R: MailboxRuntime, {
  control_ref: PriorityActorRef<M, R>,
  map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  watcher: Option<ActorId>,
}

/// Guardian: 子アクター群を監督し、SystemMessage を送出する。
pub struct Guardian<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime,
  Strat: GuardianStrategy<M, R>, {
  next_id: usize,
  children: BTreeMap<ActorId, ChildRecord<M, R>>,
  strategy: Strat,
  _marker: core::marker::PhantomData<M>,
}

impl<M, R, Strat> Guardian<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
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

  pub fn register_child(
    &mut self,
    control_ref: PriorityActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
    watcher: Option<ActorId>,
  ) -> Result<ActorId, QueueError<PriorityEnvelope<M>>> {
    let id = ActorId(self.next_id);
    self.next_id += 1;
    self.strategy.before_start(id);
    self.children.insert(
      id,
      ChildRecord {
        control_ref: control_ref.clone(),
        map_system: map_system.clone(),
        watcher,
      },
    );

    if let Some(watcher_id) = watcher {
      let map_clone = map_system.clone();
      let envelope = PriorityEnvelope::from_system(SystemMessage::Watch(watcher_id)).map(move |sys| (map_clone)(sys));
      control_ref.sender().try_send(envelope)?;
    }

    Ok(id)
  }

  pub fn remove_child(&mut self, id: ActorId) -> Option<PriorityActorRef<M, R>> {
    self.children.remove(&id).map(|record| {
      if let Some(watcher_id) = record.watcher {
        let map_clone = record.map_system.clone();
        let envelope =
          PriorityEnvelope::from_system(SystemMessage::Unwatch(watcher_id)).map(move |sys| (map_clone)(sys));
        let _ = record.control_ref.sender().try_send(envelope);
      }
      record.control_ref
    })
  }

  pub fn child_ref(&self, id: ActorId) -> Option<&PriorityActorRef<M, R>> {
    self.children.get(&id).map(|record| &record.control_ref)
  }

  pub fn notify_failure(
    &mut self,
    actor: ActorId,
    error: &dyn fmt::Debug,
  ) -> Result<Option<FailureInfo>, QueueError<PriorityEnvelope<M>>> {
    let failure = FailureInfo::from_error(actor, error);
    let directive = self.strategy.decide(actor, error);
    match (directive, self.children.get(&actor)) {
      (SupervisorDirective::Resume, _) => Ok(None),
      (SupervisorDirective::Stop, Some(record)) => {
        let envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(|sys| (record.map_system)(sys));
        record.control_ref.sender().try_send(envelope)?;
        Ok(None)
      }
      (SupervisorDirective::Restart, Some(record)) => {
        let envelope = PriorityEnvelope::from_system(SystemMessage::Restart).map(|sys| (record.map_system)(sys));
        record.control_ref.sender().try_send(envelope)?;
        self.strategy.after_restart(actor);
        Ok(None)
      }
      (SupervisorDirective::Escalate, _) => Ok(Some(failure)),
      (_, None) => Ok(Some(failure)),
    }
  }

  pub fn stop_child(&mut self, actor: ActorId) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    if let Some(record) = self.children.get(&actor) {
      let envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(|sys| (record.map_system)(sys));
      record.control_ref.sender().try_send(envelope)
    } else {
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor_id::ActorId;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use crate::mailbox::{PriorityChannel, PriorityEnvelope};
  use nexus_utils_core_rs::DEFAULT_PRIORITY;

  #[test]
  fn guardian_sends_restart_message() {
    let runtime = TestMailboxRuntime::unbounded();
    let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
    let ref_control: PriorityActorRef<SystemMessage, TestMailboxRuntime> = PriorityActorRef::new(sender);

    let mut guardian: Guardian<SystemMessage, _, AlwaysRestart> = Guardian::new(AlwaysRestart);
    let parent_id = ActorId(1);
    let actor_id = guardian
      .register_child(ref_control.clone(), Arc::new(|sys| sys), Some(parent_id))
      .unwrap();

    let first_envelope = mailbox.queue().poll().unwrap().unwrap();
    assert_eq!(first_envelope.into_parts().0, SystemMessage::Watch(parent_id));

    assert!(guardian.notify_failure(actor_id, &"panic").unwrap().is_none());

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
    let parent_id = ActorId(7);
    let actor_id = guardian
      .register_child(ref_control.clone(), Arc::new(|sys| sys), Some(parent_id))
      .unwrap();

    let watch_envelope = mailbox.queue().poll().unwrap().unwrap();
    assert_eq!(watch_envelope.into_parts().0, SystemMessage::Watch(parent_id));

    assert!(guardian.notify_failure(actor_id, &"panic").unwrap().is_none());

    let envelope = mailbox.queue().poll().unwrap().unwrap();
    assert_eq!(envelope.into_parts().0, SystemMessage::Stop);
  }

  #[test]
  fn guardian_emits_unwatch_on_remove() {
    let runtime = TestMailboxRuntime::unbounded();
    let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
    let ref_control: PriorityActorRef<SystemMessage, TestMailboxRuntime> = PriorityActorRef::new(sender);

    let mut guardian: Guardian<SystemMessage, _, AlwaysRestart> = Guardian::new(AlwaysRestart);
    let parent_id = ActorId(3);
    let actor_id = guardian
      .register_child(ref_control.clone(), Arc::new(|sys| sys), Some(parent_id))
      .unwrap();

    // consume watch message
    let _ = mailbox.queue().poll().unwrap().unwrap();

    let _ = guardian.remove_child(actor_id);

    let envelope = mailbox.queue().poll().unwrap().unwrap();
    assert_eq!(envelope.into_parts().0, SystemMessage::Unwatch(parent_id));
  }
}
