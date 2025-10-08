use super::*;
use crate::runtime::context::InternalActorRef;
use crate::runtime::mailbox::test_support::TestMailboxFactory;
use crate::runtime::mailbox::PriorityChannel;
use crate::ActorId;
use crate::ActorPath;
use crate::MailboxFactory;
use crate::SupervisorDirective;
use crate::{PriorityEnvelope, SystemMessage};
use ::core::fmt;
use alloc::sync::Arc;
use nexus_utils_core_rs::{Element, DEFAULT_PRIORITY};

#[test]
fn guardian_sends_restart_message() {
  let runtime = TestMailboxFactory::unbounded();
  let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
  let ref_control: InternalActorRef<SystemMessage, TestMailboxFactory> = InternalActorRef::new(sender);

  let mut guardian: Guardian<SystemMessage, _, AlwaysRestart> = Guardian::new(AlwaysRestart);
  let parent_id = ActorId(1);
  let parent_path = ActorPath::new();
  let (actor_id, _path) = guardian
    .register_child(ref_control.clone(), Arc::new(|sys| sys), Some(parent_id), &parent_path)
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
    R: MailboxFactory,
  {
    fn decide(&mut self, _actor: ActorId, _error: &dyn fmt::Debug) -> SupervisorDirective {
      SupervisorDirective::Stop
    }
  }

  let runtime = TestMailboxFactory::unbounded();
  let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
  let ref_control: InternalActorRef<SystemMessage, TestMailboxFactory> = InternalActorRef::new(sender);

  let mut guardian: Guardian<SystemMessage, _, AlwaysStop> = Guardian::new(AlwaysStop);
  let parent_id = ActorId(7);
  let parent_path = ActorPath::new();
  let (actor_id, _path) = guardian
    .register_child(ref_control.clone(), Arc::new(|sys| sys), Some(parent_id), &parent_path)
    .unwrap();

  let watch_envelope = mailbox.queue().poll().unwrap().unwrap();
  assert_eq!(watch_envelope.into_parts().0, SystemMessage::Watch(parent_id));

  assert!(guardian.notify_failure(actor_id, &"panic").unwrap().is_none());

  let envelope = mailbox.queue().poll().unwrap().unwrap();
  assert_eq!(envelope.into_parts().0, SystemMessage::Stop);
}

#[test]
fn guardian_emits_unwatch_on_remove() {
  let runtime = TestMailboxFactory::unbounded();
  let (mailbox, sender) = runtime.build_default_mailbox::<PriorityEnvelope<SystemMessage>>();
  let ref_control: InternalActorRef<SystemMessage, TestMailboxFactory> = InternalActorRef::new(sender);

  let mut guardian: Guardian<SystemMessage, _, AlwaysRestart> = Guardian::new(AlwaysRestart);
  let parent_id = ActorId(3);
  let parent_path = ActorPath::new();
  let (actor_id, _path) = guardian
    .register_child(ref_control.clone(), Arc::new(|sys| sys), Some(parent_id), &parent_path)
    .unwrap();

  // consume watch message
  let _ = mailbox.queue().poll().unwrap().unwrap();

  let _ = guardian.remove_child(actor_id);

  let envelope = mailbox.queue().poll().unwrap().unwrap();
  assert_eq!(envelope.into_parts().0, SystemMessage::Unwatch(parent_id));
}
