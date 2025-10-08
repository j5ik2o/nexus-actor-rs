#![allow(deprecated)]

use super::*;
use crate::actor_id::ActorId;
use crate::context::ActorContext;
use crate::guardian::AlwaysRestart;
use crate::mailbox::test_support::TestMailboxRuntime;
use crate::mailbox::SystemMessage;
use crate::MailboxOptions;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(feature = "std")]
use futures::executor::block_on;

#[cfg(feature = "std")]
#[test]
fn typed_actor_system_handles_user_messages() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(runtime);

  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
    log_clone.borrow_mut().push(msg);
  });

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn typed actor");
  actor_ref.tell(11).expect("tell");
  block_on(root.dispatch_next()).expect("dispatch");

  assert_eq!(log.borrow().as_slice(), &[11]);
}

#[cfg(feature = "std")]
#[test]
fn test_typed_actor_handles_system_stop() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(runtime);

  let stopped: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
  let stopped_clone = stopped.clone();

  let system_handler = move |_: &mut ActorContext<'_, MessageEnvelope<u32>, _, _>, sys_msg: SystemMessage| {
    if matches!(sys_msg, SystemMessage::Stop) {
      *stopped_clone.borrow_mut() = true;
    }
  };

  let props = Props::with_system_handler(MailboxOptions::default(), move |_, _msg: u32| {}, Some(system_handler));

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn typed actor");
  actor_ref.send_system(SystemMessage::Stop).expect("send stop");
  block_on(root.dispatch_next()).expect("dispatch");

  assert!(*stopped.borrow(), "SystemMessage::Stop should be handled");
}

#[cfg(feature = "std")]
#[test]
fn test_typed_actor_handles_watch_unwatch() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(runtime);

  let watchers_count: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
  let watchers_count_clone = watchers_count.clone();

  let behavior = Behavior::stateless(move |ctx: &mut Context<'_, '_, u32, _>, _msg: u32| {
    *watchers_count_clone.borrow_mut() = ctx.watchers().len();
  });

  let system_handler = Some(
    |ctx: &mut ActorContext<'_, _, _, _>, sys_msg: SystemMessage| match sys_msg {
      SystemMessage::Watch(watcher) => {
        ctx.register_watcher(watcher);
      }
      SystemMessage::Unwatch(watcher) => {
        ctx.unregister_watcher(watcher);
      }
      _ => {}
    },
  );

  let props = Props::with_behavior_and_system(MailboxOptions::default(), behavior, system_handler);

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn typed actor");

  // Get initial watcher count (parent is automatically registered)
  actor_ref.tell(1).expect("tell");
  block_on(root.dispatch_next()).expect("dispatch initial");
  let initial_count = *watchers_count.borrow();

  let watcher_id = ActorId(999);
  actor_ref
    .send_system(SystemMessage::Watch(watcher_id))
    .expect("send watch");
  block_on(root.dispatch_next()).expect("dispatch watch");

  actor_ref.tell(2).expect("tell");
  block_on(root.dispatch_next()).expect("dispatch user message");

  let after_watch_count = *watchers_count.borrow();
  assert_eq!(
    after_watch_count,
    initial_count + 1,
    "Watcher count should increase by 1"
  );

  actor_ref
    .send_system(SystemMessage::Unwatch(watcher_id))
    .expect("send unwatch");
  block_on(root.dispatch_next()).expect("dispatch unwatch");

  actor_ref.tell(3).expect("tell");
  block_on(root.dispatch_next()).expect("dispatch user message");

  let after_unwatch_count = *watchers_count.borrow();
  assert_eq!(
    after_unwatch_count, initial_count,
    "Watcher count should return to initial"
  );
}

#[cfg(feature = "std")]
#[test]
fn test_typed_actor_stateful_behavior_with_system_message() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(runtime);

  // Stateful behavior: count user messages and track system messages
  let count = Rc::new(RefCell::new(0u32));
  let failures = Rc::new(RefCell::new(0u32));

  let count_clone = count.clone();
  let behavior = Behavior::stateless(move |_ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
    *count_clone.borrow_mut() += msg;
  });

  let failures_clone = failures.clone();
  let system_handler = move |_ctx: &mut ActorContext<'_, MessageEnvelope<u32>, _, _>, sys_msg: SystemMessage| {
    if matches!(sys_msg, SystemMessage::Suspend) {
      *failures_clone.borrow_mut() += 1;
    }
  };

  let props = Props::with_behavior_and_system(MailboxOptions::default(), behavior, Some(system_handler));

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn typed actor");

  // Send user messages
  actor_ref.tell(10).expect("tell 10");
  block_on(root.dispatch_next()).expect("dispatch user 1");

  actor_ref.tell(5).expect("tell 5");
  block_on(root.dispatch_next()).expect("dispatch user 2");

  // Send system message (Suspend doesn't stop the actor)
  actor_ref.send_system(SystemMessage::Suspend).expect("send suspend");
  block_on(root.dispatch_next()).expect("dispatch system");

  // Verify stateful behavior updated correctly
  assert_eq!(*count.borrow(), 15, "State should accumulate user messages");
  assert_eq!(*failures.borrow(), 1, "State should track system messages");
}
