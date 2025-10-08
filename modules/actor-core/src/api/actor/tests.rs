#![cfg(feature = "std")]
#![allow(deprecated)]

use super::ask::create_ask_handles;
use super::behavior::{Signal, SupervisorStrategyConfig};
use super::*;
use super::{ask_with_timeout, AskError};
use crate::api::guardian::AlwaysRestart;
use crate::runtime::mailbox::test_support::TestMailboxFactory;
use crate::ActorId;
use crate::MailboxOptions;
use crate::SystemMessage;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};
use nexus_utils_core_rs::Element;
use nexus_utils_core_rs::QueueError;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Clone, Debug)]
struct ParentMessage(String);

#[derive(Clone, Debug)]
struct ChildMessage {
  text: String,
}

impl Element for ParentMessage {}
impl Element for ChildMessage {}

use futures::executor::block_on;
use futures::future;

#[test]
fn test_supervise_builder_sets_strategy() {
  let props = Props::with_behavior(MailboxOptions::default(), || {
    Behaviors::supervise(Behavior::stateless(
      |_: &mut Context<'_, '_, u32, TestMailboxFactory>, _: u32| {},
    ))
    .with_strategy(SupervisorStrategy::Restart)
  });
  let (_, supervisor_cfg) = props.into_parts();
  assert_eq!(
    supervisor_cfg,
    SupervisorStrategyConfig::from_strategy(SupervisorStrategy::Restart)
  );
}

#[test]
#[ignore = "panic handling for supervised restarts/stops not yet fully wired"]
fn test_supervise_stop_on_failure() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let props = Props::with_behavior(MailboxOptions::default(), || {
    Behaviors::supervise(Behaviors::receive(|_, _: u32| {
      panic!("boom");
    }))
    .with_strategy(SupervisorStrategy::Stop)
  });
  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.tell(1).expect("send message");
  let panic_result = catch_unwind(AssertUnwindSafe(|| {
    block_on(root.dispatch_next()).expect("dispatch failure");
  }));
  assert!(panic_result.is_err(), "expected actor to panic under Stop strategy");

  let result = actor_ref.tell(2);
  assert!(matches!(
    result,
    Err(QueueError::Closed(_)) | Err(QueueError::Disconnected)
  ));
}

#[test]
#[ignore = "panic handling for supervised restarts/resume not yet fully wired"]
fn test_supervise_resume_on_failure() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));

  let props = Props::with_behavior(MailboxOptions::default(), {
    let log_factory = log.clone();
    move || {
      let log_clone = log_factory.clone();
      Behaviors::supervise(Behaviors::receive(move |_, msg: u32| {
        if msg == 0 {
          panic!("fail once");
        }
        log_clone.borrow_mut().push(msg);
        Behaviors::same()
      }))
      .with_strategy(SupervisorStrategy::Resume)
    }
  });
  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.tell(0).expect("send failure message");
  let panic_result = catch_unwind(AssertUnwindSafe(|| {
    block_on(root.dispatch_next()).expect("dispatch failure");
  }));
  assert!(panic_result.is_err(), "expected actor to panic before resume");

  actor_ref.tell(42).expect("send second message");
  block_on(root.dispatch_next()).expect("process resume");

  assert_eq!(log.borrow().as_slice(), &[42]);
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

  let props = Props::with_behavior_and_system(
    MailboxOptions::default(),
    {
      let watchers_factory = watchers_count_clone.clone();
      move || {
        let watchers_clone = watchers_factory.clone();
        Behavior::stateless(move |ctx: &mut Context<'_, '_, u32, _>, _msg: u32| {
          *watchers_clone.borrow_mut() = ctx.watchers().len();
        })
      }
    },
    system_handler,
  );

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

  let failures_clone = failures.clone();
  let system_handler = move |_ctx: &mut Context<'_, '_, u32, _>, sys_msg: SystemMessage| {
    if matches!(sys_msg, SystemMessage::Suspend) {
      *failures_clone.borrow_mut() += 1;
    }
  };

  let props = Props::with_behavior_and_system(
    MailboxOptions::default(),
    {
      let count_factory = count.clone();
      move || {
        let count_clone = count_factory.clone();
        Behavior::stateless(move |_ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
          *count_clone.borrow_mut() += msg;
        })
      }
    },
    Some(system_handler),
  );

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

  let props = Props::with_behavior(MailboxOptions::default(), {
    let log_factory = log.clone();
    move || {
      let log_clone = log_factory.clone();
      Behaviors::receive(move |ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
        log_clone.borrow_mut().push(msg);
        if msg < 2 {
          ctx.self_ref().tell(msg + 1).expect("self tell");
        }
        Behaviors::same()
      })
    }
  });

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

  let props = Props::with_behavior(MailboxOptions::default(), {
    let child_log_factory = child_log_for_parent.clone();
    move || {
      let child_log_parent = child_log_factory.clone();
      Behaviors::receive(move |ctx: &mut Context<'_, '_, ParentMessage, _>, msg: ParentMessage| {
        let name = msg.0;
        let child_props = Props::with_behavior(MailboxOptions::default(), {
          let child_log_factory = child_log_parent.clone();
          move || {
            let child_log_for_child = child_log_factory.clone();
            Behaviors::receive(
              move |_child_ctx: &mut Context<'_, '_, ChildMessage, _>, child_msg: ChildMessage| {
                child_log_for_child.borrow_mut().push(child_msg.text.clone());
                Behaviors::same()
              },
            )
          }
        });
        let child_ref = ctx.spawn_child(child_props);
        child_ref
          .tell(ChildMessage {
            text: format!("hello {name}"),
          })
          .expect("tell child");
        Behaviors::same()
      })
    }
  });
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
  let adapter_slot: Rc<RefCell<Option<MessageAdapterRef<String, u32, _>>>> = Rc::new(RefCell::new(None));

  let props = Props::with_behavior(MailboxOptions::default(), {
    let log_factory = log.clone();
    let adapter_slot_factory = adapter_slot.clone();
    move || {
      let log_clone = log_factory.clone();
      let adapter_slot_clone = adapter_slot_factory.clone();
      Behaviors::receive(move |ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
        log_clone.borrow_mut().push(msg);
        if adapter_slot_clone.borrow().is_none() {
          let adapter = ctx.message_adapter(|text: String| text.len() as u32);
          adapter_slot_clone.borrow_mut().replace(adapter);
        }
        Behaviors::same()
      })
    }
  });
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

  let props = Props::with_behavior(MailboxOptions::default(), {
    let child_log_factory = child_log_for_parent.clone();
    let child_ref_holder_factory = child_ref_holder_clone.clone();
    move || {
      let child_log_parent = child_log_factory.clone();
      let child_ref_holder_local = child_ref_holder_factory.clone();
      Behavior::stateless(move |ctx: &mut Context<'_, '_, u32, _>, msg: u32| {
        if child_ref_holder_local.borrow().is_none() {
          let child_log_for_child = child_log_parent.clone();
          let child_props = Props::new(MailboxOptions::default(), move |_, child_msg: u32| {
            child_log_for_child.borrow_mut().push(child_msg);
          });
          let child_ref = ctx.spawn_child(child_props);
          child_ref_holder_local.borrow_mut().replace(child_ref);
        }

        if let Some(child_ref) = child_ref_holder_local.borrow().clone() {
          child_ref.tell(msg * 2).expect("tell child");
        }
      })
    }
  });

  let mut root = system.root_context();
  let parent_ref = root.spawn(props).expect("spawn parent actor");

  parent_ref.tell(3).expect("tell parent");
  block_on(root.dispatch_next()).expect("dispatch parent");
  block_on(root.dispatch_next()).expect("dispatch child");

  assert_eq!(child_log.borrow().as_slice(), &[6]);
}

#[test]
fn test_behaviors_setup_spawns_named_child() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<String, _, AlwaysRestart> = ActorSystem::new(factory);

  let child_log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

  let props = Props::with_behavior(MailboxOptions::default(), {
    let child_log_factory = child_log.clone();
    move || {
      let child_log_parent = child_log_factory.clone();
      Behaviors::setup(move |ctx| {
        let child_props = Props::with_behavior(MailboxOptions::default(), {
          let child_log_clone = child_log_parent.clone();
          move || {
            let log_ref = child_log_clone.clone();
            Behavior::stateless(move |_, msg: String| {
              log_ref.borrow_mut().push(msg);
            })
          }
        });
        let greeter = ctx.spawn_child(child_props);
        Behaviors::receive(move |_, msg: String| {
          greeter.tell(format!("hello {msg}")).expect("forward to child");
          Behaviors::same()
        })
      })
    }
  });

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.tell("world".to_string()).expect("tell message");
  block_on(root.dispatch_next()).expect("dispatch setup+message");
  block_on(root.dispatch_next()).expect("dispatch child");

  assert_eq!(child_log.borrow().as_slice(), &["hello world".to_string()]);
}

#[test]
fn test_receive_signal_post_stop() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: ActorSystem<u32, _, AlwaysRestart> = ActorSystem::new(factory);

  let signals: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
  let signals_clone = signals.clone();

  let props = Props::with_behavior(MailboxOptions::default(), move || {
    let signals_cell = signals_clone.clone();
    Behaviors::receive(|_, msg: u32| {
      if msg == 0 {
        Behaviors::transition(Behaviors::stopped())
      } else {
        Behaviors::same()
      }
    })
    .receive_signal(move |_, signal| {
      match signal {
        Signal::PostStop => signals_cell.borrow_mut().push("post_stop"),
      }
      Behaviors::same()
    })
  });

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.send_system(SystemMessage::Stop).expect("send stop");
  block_on(root.dispatch_next()).expect("dispatch stop");
  let _ = block_on(root.dispatch_next());

  assert_eq!(signals.borrow().as_slice(), &["post_stop"]);
}

fn noop_waker() -> Waker {
  fn clone(_: *const ()) -> RawWaker {
    noop_raw_waker()
  }
  fn wake(_: *const ()) {}
  fn wake_by_ref(_: *const ()) {}
  fn drop(_: *const ()) {}

  fn noop_raw_waker() -> RawWaker {
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn resolve<F>(mut future: F) -> F::Output
where
  F: Future + Unpin, {
  let waker = noop_waker();
  let mut future = Pin::new(&mut future);
  let mut cx = TaskContext::from_waker(&waker);
  loop {
    match future.as_mut().poll(&mut cx) {
      Poll::Ready(value) => return value,
      Poll::Pending => core::hint::spin_loop(),
    }
  }
}

#[test]
fn ask_future_completes_successfully() {
  let (future, responder) = create_ask_handles::<u32>();
  responder.dispatch_user(7_u32).expect("dispatch succeeds");

  let result = resolve(future);
  assert_eq!(result.expect("ask result"), 7);
}

#[test]
fn ask_future_timeout_returns_error() {
  let (future, _responder) = create_ask_handles::<u32>();
  let timed = ask_with_timeout(future, future::ready(()));

  let result = resolve(timed);
  assert!(
    matches!(result, Err(AskError::Timeout)),
    "unexpected result: {:?}",
    result
  );
}

#[test]
fn ask_future_responder_drop_propagates() {
  let (future, responder) = create_ask_handles::<u32>();
  drop(responder);

  let result = resolve(future);
  assert!(matches!(result, Err(AskError::ResponderDropped)));
}

#[test]
fn ask_future_cancelled_on_drop() {
  let (future, responder) = create_ask_handles::<u32>();
  drop(future);
  drop(responder);
}
