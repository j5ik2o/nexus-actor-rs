use super::*;
use nexus_utils_core_rs::{QueueSize, DEFAULT_PRIORITY};

#[test]
fn mailbox_options_helpers_cover_basic_cases() {
  let limited = MailboxOptions::with_capacity(4);
  assert_eq!(limited.capacity, QueueSize::limited(4));
  assert!(!limited.capacity.is_limitless());

  let unbounded = MailboxOptions::unbounded();
  assert!(unbounded.capacity.is_limitless());

  let defaulted = MailboxOptions::default();
  assert_eq!(defaulted.capacity, QueueSize::limitless());
}

#[test]
fn priority_envelope_exposes_priority() {
  let envelope = PriorityEnvelope::new(42_u8, 5);
  assert_eq!(envelope.priority(), 5);
  assert_eq!(*envelope.message(), 42);
  let raised = PriorityEnvelope::new(42_u8, 5).map_priority(|p| p + 1);
  assert_eq!(raised.priority(), 6);

  let default = PriorityEnvelope::with_default_priority(7_u8);
  assert_eq!(default.priority(), DEFAULT_PRIORITY);

  let (message, priority) = PriorityEnvelope::new(42_u8, 5).into_parts();
  assert_eq!(message, 42);
  assert_eq!(priority, 5);

  let control = PriorityEnvelope::control(99_u8, 9);
  assert!(control.is_control());
  let (msg, pri, channel) = control.into_parts_with_channel();
  assert_eq!(msg, 99);
  assert_eq!(pri, 9);
  assert_eq!(channel, PriorityChannel::Control);

  let sys_env = PriorityEnvelope::<SystemMessage>::from_system(SystemMessage::Stop);
  assert!(sys_env.is_control());
  assert!(sys_env.priority() > DEFAULT_PRIORITY);
}
