#![cfg(feature = "std")]
#![allow(deprecated)]

use super::behavior::SupervisorStrategyConfig;
use super::*;
use crate::api::guardian::AlwaysRestart;
use crate::runtime::mailbox::test_support::TestMailboxFactory;
use crate::ActorId;
use crate::MailboxOptions;
use crate::SystemMessage;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use nexus_utils_core_rs::Element;

#[derive(Clone, Debug)]
struct ParentMessage(String);

#[derive(Clone, Debug)]
struct ChildMessage {
  text: String,
}

impl Element for ParentMessage {}
impl Element for ChildMessage {}

use futures::executor::block_on;

#[test]
fn test_supervise_builder_sets_strategy() {
  let behavior = Behaviors::supervise(Behavior::stateless(
    |_: &mut Context<'_, '_, u32, TestMailboxFactory>, _: u32| {},
  ))
  .with_strategy(SupervisorStrategy::Restart);

  let props = Props::with_behavior(MailboxOptions::default(), behavior);
  let (_, supervisor_cfg) = props.into_parts();
  assert_eq!(
    supervisor_cfg,
    SupervisorStrategyConfig::from_strategy(SupervisorStrategy::Restart)
  );
}

#[test]
fn typed_actor_system_handles_user_messages() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

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

#[test]
fn test_typed_actor_handles_system_stop() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let stopped: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
  let stopped_clone = stopped.clone();

  let system_handler = move |_: &mut Context<'_, '_, u32, _>, sys_msg: SystemMessage| {
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

#[test]
fn test_typed_actor_handles_watch_unwatch() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let watchers_count: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
  let watchers_count_clone = watchers_count.clone();

  let behavior = Behavior::stateless(move |ctx: &mut Context<'_, '_, u32, _>, _msg: u32| {
    *watchers_count_clone.borrow_mut() = ctx.watchers().len();
  });

  let system_handler = Some(
    |ctx: &mut Context<'_, '_, u32, _>, sys_msg: SystemMessage| match sys_msg {
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
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  // Stateful behavior: count user messages and track system messages
  let count = Rc::new(RefCell::new(0u32));
  let failures = Rc::new(RefCell::new(0u32));

  let count_clone = count.clone();
  let behavior = Behavior::stateless(move |_ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
    *count_clone.borrow_mut() += msg;
  });

  let failures_clone = failures.clone();
  let system_handler = move |_ctx: &mut Context<'_, '_, u32, _>, sys_msg: SystemMessage| {
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

#[test]
fn test_behaviors_receive_self_loop() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let behavior = Behaviors::receive(move |ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
    log_clone.borrow_mut().push(msg);
    if msg < 2 {
      ctx.self_ref().tell(msg + 1).expect("self tell");
    }
    Behaviors::same()
  });

  let props = Props::with_behavior(MailboxOptions::default(), behavior);

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.tell(0).expect("tell initial");
  block_on(root.dispatch_next()).expect("process 0");
  block_on(root.dispatch_next()).expect("process 1");
  block_on(root.dispatch_next()).expect("process 2");

  assert_eq!(log.borrow().as_slice(), &[0, 1, 2]);
}

#[test]
fn test_parent_spawns_child_with_distinct_message_type() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<ParentMessage, _, AlwaysRestart> = ActorSystem::new(factory);

  let child_log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
  let child_log_for_parent = child_log.clone();

  let parent_behavior = Behaviors::receive(move |ctx: &mut Context<'_, '_, ParentMessage, _>, msg: ParentMessage| {
    let name = msg.0;
    let child_log_for_child = child_log_for_parent.clone();
    let child_behavior = Behaviors::receive(
      move |_child_ctx: &mut Context<'_, '_, ChildMessage, _>, child_msg: ChildMessage| {
        child_log_for_child.borrow_mut().push(child_msg.text.clone());
        Behaviors::same()
      },
    );
    let child_props = Props::with_behavior(MailboxOptions::default(), child_behavior);
    let child_ref = ctx.spawn_child(child_props);
    child_ref
      .tell(ChildMessage {
        text: format!("hello {name}"),
      })
      .expect("tell child");
    Behaviors::same()
  });

  let props = Props::with_behavior(MailboxOptions::default(), parent_behavior);
  let mut root = system.root_context();
  let parent_ref = root.spawn(props).expect("spawn parent");

  parent_ref
    .tell(ParentMessage("world".to_string()))
    .expect("tell parent");
  block_on(root.dispatch_next()).expect("dispatch parent");
  block_on(root.dispatch_next()).expect("dispatch child");

  assert_eq!(child_log.borrow().as_slice(), &["hello world".to_string()]);
}

#[test]
fn test_message_adapter_converts_external_message() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();
  let adapter_slot: Rc<RefCell<Option<MessageAdapterRef<String, u32, _>>>> = Rc::new(RefCell::new(None));
  let adapter_slot_clone = adapter_slot.clone();

  let behavior = Behaviors::receive(move |ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
    log_clone.borrow_mut().push(msg);
    if adapter_slot_clone.borrow().is_none() {
      let adapter = ctx.message_adapter(|text: String| text.len() as u32);
      adapter_slot_clone.borrow_mut().replace(adapter);
    }
    Behaviors::same()
  });

  let props = Props::with_behavior(MailboxOptions::default(), behavior);
  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.tell(1).expect("initial message");
  block_on(root.dispatch_next()).expect("dispatch primary");

  let adapter = adapter_slot.borrow().as_ref().cloned().expect("adapter must exist");
  adapter.tell("abcd".to_string()).expect("adapter tell");
  block_on(root.dispatch_next()).expect("dispatch adapted");

  assert_eq!(log.borrow().as_slice(), &[1, 4]);
}

#[test]
fn test_parent_actor_spawns_child() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let child_log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let child_log_for_parent = child_log.clone();
  let child_ref_holder: Rc<RefCell<Option<ActorRef<u32, _>>>> = Rc::new(RefCell::new(None));
  let child_ref_holder_clone = child_ref_holder.clone();

  let parent_behavior = Behavior::stateless(move |ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
    if child_ref_holder_clone.borrow().is_none() {
      let child_log_for_child = child_log_for_parent.clone();
      let child_props = Props::new(MailboxOptions::default(), move |_, child_msg: u32| {
        child_log_for_child.borrow_mut().push(child_msg);
      });
      let child_ref = ctx.spawn_child(child_props);
      child_ref_holder_clone.borrow_mut().replace(child_ref);
    }

    let maybe_child = { child_ref_holder_clone.borrow().clone() };
    if let Some(child_ref) = maybe_child {
      child_ref.tell(msg * 2).expect("tell child");
    }
  });

  let props = Props::with_behavior(MailboxOptions::default(), parent_behavior);

  let mut root = system.root_context();
  let parent_ref = root.spawn(props).expect("spawn parent actor");

  parent_ref.tell(3).expect("tell parent");
  block_on(root.dispatch_next()).expect("dispatch parent");
  block_on(root.dispatch_next()).expect("dispatch child");

  assert_eq!(child_log.borrow().as_slice(), &[6]);
}
