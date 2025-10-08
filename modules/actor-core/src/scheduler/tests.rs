#![allow(deprecated)]
use super::*;
use crate::actor_id::ActorId;
use crate::context::PriorityActorRef;
use crate::failure::FailureInfo;
use crate::guardian::{AlwaysRestart, GuardianStrategy};
use crate::mailbox::test_support::TestMailboxRuntime;
use crate::mailbox::{MailboxOptions, SystemMessage};
use crate::supervisor::NoopSupervisor;
use crate::{MailboxRuntime, PriorityEnvelope};
#[cfg(feature = "std")]
use crate::SupervisorDirective;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::cell::Cell;
use core::cell::RefCell;
#[cfg(feature = "std")]
use futures::executor::block_on;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug)]
struct AlwaysEscalate;

#[cfg(feature = "std")]
impl<M, R> GuardianStrategy<M, R> for AlwaysEscalate
where
  M: Element,
  R: MailboxRuntime,
{
  fn decide(&mut self, _actor: ActorId, _error: &dyn core::fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Escalate
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Message {
  User(u32),
  System(SystemMessage),
}

impl nexus_utils_core_rs::Element for Message {}

#[test]
fn scheduler_delivers_watch_before_user_messages() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let _actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| {
        log_clone.borrow_mut().push(msg.clone());
      },
    )
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();

  assert_eq!(
    log.borrow().as_slice(),
    &[Message::System(SystemMessage::Watch(ActorId::ROOT))]
  );
}

#[test]
fn actor_context_exposes_parent_watcher() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler = PriorityScheduler::new(runtime);

  let watchers_log: Rc<RefCell<Vec<Vec<ActorId>>>> = Rc::new(RefCell::new(Vec::new()));
  let watchers_clone = watchers_log.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |ctx, msg: Message| {
        let current_watchers = ctx.watchers().to_vec();
        watchers_clone.borrow_mut().push(current_watchers);
        match msg {
          Message::User(_) => {}
          Message::System(_) => {}
        }
      },
    )
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();
  assert_eq!(watchers_log.borrow().as_slice(), &[vec![ActorId::ROOT]]);

  actor_ref
    .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
    .unwrap();
  block_on(scheduler.dispatch_next()).unwrap();

  assert_eq!(
    watchers_log.borrow().as_slice(),
    &[vec![ActorId::ROOT], vec![ActorId::ROOT]]
  );
}

#[test]
fn scheduler_dispatches_high_priority_first() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<(u32, i8)>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |ctx, msg: Message| match msg {
        Message::User(value) => {
          log_clone.borrow_mut().push((value, ctx.current_priority().unwrap()));
          if value == 99 {
            let child_log = log_clone.clone();
            ctx
              .spawn_child(
                NoopSupervisor,
                MailboxOptions::default(),
                move |_, child_msg: Message| {
                  if let Message::User(child_value) = child_msg {
                    child_log.borrow_mut().push((child_value, 0));
                  }
                },
              )
              .try_send_with_priority(Message::User(7), 0)
              .unwrap();
          }
        }
        Message::System(_) => {}
      },
    )
    .unwrap();

  actor_ref.try_send_with_priority(Message::User(10), 1).unwrap();
  actor_ref.try_send_with_priority(Message::User(99), 7).unwrap();
  actor_ref.try_send_with_priority(Message::User(20), 3).unwrap();

  block_on(scheduler.dispatch_next()).unwrap();
  block_on(scheduler.dispatch_next()).unwrap();

  assert_eq!(scheduler.actor_count(), 2);

  assert_eq!(log.borrow().as_slice(), &[(99, 7), (20, 3), (10, 1), (7, 0)]);
}

#[test]
fn scheduler_prioritizes_system_messages() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| {
        log_clone.borrow_mut().push(msg.clone());
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(42), DEFAULT_PRIORITY)
    .unwrap();

  let control_envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(Message::System);
  actor_ref.try_send_envelope(control_envelope).unwrap();

  block_on(scheduler.dispatch_next()).unwrap();

  assert_eq!(
    log.borrow().as_slice(),
    &[
      Message::System(SystemMessage::Stop),
      Message::System(SystemMessage::Watch(ActorId::ROOT)),
      Message::User(42),
    ]
  );
}

#[test]
fn priority_actor_ref_sends_system_messages() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<SystemMessage, _> = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<SystemMessage>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| sys),
      move |_, msg: SystemMessage| {
        log_clone.borrow_mut().push(msg.clone());
      },
    )
    .unwrap();

  actor_ref.try_send_system(SystemMessage::Restart).unwrap();
  block_on(scheduler.dispatch_next()).unwrap();

  assert_eq!(
    log.borrow().as_slice(),
    &[SystemMessage::Restart, SystemMessage::Watch(ActorId::ROOT)]
  );
}

#[cfg(feature = "std")]
#[test]
fn scheduler_notifies_guardian_and_restarts_on_panic() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysRestart> = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();
  let should_panic = Rc::new(Cell::new(true));
  let panic_flag = should_panic.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| {
        match msg {
          Message::System(SystemMessage::Watch(_)) => {
            // Watch メッセージは監視登録のみなのでログに残さない
          }
          Message::User(_) if panic_flag.get() => {
            panic_flag.set(false);
            panic!("boom");
          }
          _ => {
            log_clone.borrow_mut().push(msg.clone());
          }
        }
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();
  assert!(log.borrow().is_empty());

  block_on(scheduler.dispatch_next()).unwrap();

  assert_eq!(log.borrow().as_slice(), &[Message::System(SystemMessage::Restart)]);
  assert!(!should_panic.get());
}

#[cfg(feature = "std")]
#[test]
fn scheduler_run_until_processes_messages() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysRestart> = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::User(value) => log_clone.borrow_mut().push(Message::User(value)),
        Message::System(_) => {}
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(11), DEFAULT_PRIORITY)
    .unwrap();

  let mut loops = 0;
  futures::executor::block_on(scheduler.run_until(|| {
    let continue_loop = loops == 0;
    loops += 1;
    continue_loop
  }))
  .unwrap();

  assert_eq!(log.borrow().as_slice(), &[Message::User(11)]);
}

#[cfg(feature = "std")]
#[test]
fn scheduler_blocking_dispatch_loop_stops_with_closure() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysRestart> = PriorityScheduler::new(runtime);

  let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::User(value) => log_clone.borrow_mut().push(Message::User(value)),
        Message::System(_) => {}
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(21), DEFAULT_PRIORITY)
    .unwrap();

  let mut loops = 0;
  scheduler
    .blocking_dispatch_loop(|| {
      let continue_loop = loops == 0;
      loops += 1;
      continue_loop
    })
    .unwrap();

  assert_eq!(log.borrow().as_slice(), &[Message::User(21)]);
}

#[cfg(feature = "std")]
#[test]
fn scheduler_records_escalations() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
    PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

  let sink: Rc<RefCell<Vec<FailureInfo>>> = Rc::new(RefCell::new(Vec::new()));
  let sink_clone = sink.clone();
  scheduler.on_escalation(move |info| {
    sink_clone.borrow_mut().push(info.clone());
    Ok(())
  });

  let should_panic = Rc::new(Cell::new(true));
  let panic_flag = should_panic.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::System(SystemMessage::Watch(_)) => {}
        Message::User(_) if panic_flag.get() => {
          panic_flag.set(false);
          panic!("boom");
        }
        _ => {}
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();

  let handler_data = sink.borrow();
  assert_eq!(handler_data.len(), 1);
  assert_eq!(handler_data[0].actor, ActorId(0));
  assert!(handler_data[0].reason.starts_with("panic:"));

  // handler で除去済みのため take_escalations は空
  assert!(scheduler.take_escalations().is_empty());
}

#[cfg(feature = "std")]
#[test]
fn scheduler_escalation_handler_delivers_to_parent() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
    PriorityScheduler::with_strategy(runtime.clone(), AlwaysEscalate);

  let (parent_mailbox, parent_sender) = runtime.build_default_mailbox::<PriorityEnvelope<Message>>();
  let parent_ref: PriorityActorRef<Message, TestMailboxRuntime> = PriorityActorRef::new(parent_sender);
  scheduler.set_parent_guardian(parent_ref, Arc::new(|sys| Message::System(sys)));

  let should_panic = Rc::new(Cell::new(true));
  let panic_flag = should_panic.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::System(SystemMessage::Watch(_)) => {}
        Message::User(_) if panic_flag.get() => {
          panic_flag.set(false);
          panic!("boom");
        }
        _ => {}
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();

  let envelope = parent_mailbox.queue().poll().unwrap().unwrap();
  let (msg, _, channel) = envelope.into_parts_with_channel();
  assert_eq!(channel, crate::mailbox::PriorityChannel::Control);
  match msg {
    Message::System(SystemMessage::Escalate(info)) => {
      assert_eq!(info.actor, ActorId(0));
      assert!(info.reason.contains("panic"));
    }
    other => panic!("unexpected message: {:?}", other),
  }
}

#[cfg(feature = "std")]
#[test]
fn scheduler_escalation_chain_reaches_root() {
  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
    PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

  let collected: Rc<RefCell<Vec<FailureInfo>>> = Rc::new(RefCell::new(Vec::new()));
  let collected_clone = collected.clone();
  scheduler.on_escalation(move |info| {
    collected_clone.borrow_mut().push(info.clone());
    Ok(())
  });

  let parent_triggered = Rc::new(Cell::new(false));
  let trigger_flag = parent_triggered.clone();
  let child_panics = Rc::new(Cell::new(true));
  let child_flag = child_panics.clone();

  let parent_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |ctx, msg: Message| match msg {
        Message::System(_) => {}
        Message::User(0) if !trigger_flag.get() => {
          trigger_flag.set(true);
          let panic_once = child_flag.clone();
          ctx
            .spawn_child(
              NoopSupervisor,
              MailboxOptions::default(),
              move |_, child_msg: Message| match child_msg {
                Message::System(_) => {}
                Message::User(1) if panic_once.get() => {
                  panic_once.set(false);
                  panic!("child failure");
                }
                _ => {}
              },
            )
            .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
            .unwrap();
        }
        _ => {}
      },
    )
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();

  {
    let snapshot = collected.borrow();
    assert_eq!(snapshot.len(), 0);
  }

  parent_ref
    .try_send_with_priority(Message::User(0), DEFAULT_PRIORITY)
    .unwrap();

  block_on(scheduler.dispatch_next()).unwrap();
  {
    let snapshot = collected.borrow();
    assert_eq!(snapshot.len(), 0);
  }

  block_on(scheduler.dispatch_next()).unwrap();
  {
    let snapshot = collected.borrow();
    assert_eq!(snapshot.len(), 1);
  }
  let first_failure = collected.borrow()[0].clone();
  let first_stage = first_failure.stage;
  assert!(first_stage.hops() >= 1, "child escalation should have hop count >= 1");

  let parent_failure = first_failure
    .escalate_to_parent()
    .expect("parent failure info must exist");
  let parent_stage = parent_failure.stage;
  assert!(
    parent_stage.hops() >= first_stage.hops(),
    "parent escalation hop count must be monotonic"
  );

  let root_failure = scheduler
    .guardian
    .escalate_failure(parent_failure.clone())
    .unwrap()
    .expect("root failure should be produced");
  let root_stage = root_failure.stage;

  assert_eq!(first_failure.path.segments().last().copied(), Some(first_failure.actor));

  assert_eq!(
    parent_failure.actor,
    first_failure
      .path
      .segments()
      .first()
      .copied()
      .unwrap_or(first_failure.actor)
  );

  assert_eq!(root_failure.actor, parent_failure.actor);
  assert!(root_failure.path.is_empty());
  assert_eq!(root_failure.reason, parent_failure.reason);
  assert!(
    root_stage.hops() >= parent_stage.hops(),
    "root escalation hop count must be monotonic"
  );
}

#[cfg(feature = "std")]
#[test]
fn scheduler_root_escalation_handler_invoked() {
  use std::sync::{Arc as StdArc, Mutex};

  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
    PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

  let events: StdArc<Mutex<Vec<FailureInfo>>> = StdArc::new(Mutex::new(Vec::new()));
  let events_clone = events.clone();

  scheduler.set_root_escalation_handler(Some(Arc::new(move |info: &FailureInfo| {
    events_clone.lock().unwrap().push(info.clone());
  })));

  let should_panic = Rc::new(Cell::new(true));
  let panic_flag = should_panic.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::System(SystemMessage::Watch(_)) => {}
        Message::User(_) if panic_flag.get() => {
          panic_flag.set(false);
          panic!("root boom");
        }
        _ => {}
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(42), DEFAULT_PRIORITY)
    .unwrap();

  let events_ref = events.clone();
  block_on(scheduler.run_until(|| events_ref.lock().unwrap().is_empty())).unwrap();

  let events = events.lock().unwrap();
  assert_eq!(events.len(), 1);
  assert!(!events[0].reason.is_empty());
}

#[cfg(feature = "std")]
#[test]
fn scheduler_requeues_failed_custom_escalation() {
  use core::cell::Cell;

  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
    PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

  let attempts = Rc::new(Cell::new(0usize));
  let attempts_clone = attempts.clone();
  scheduler.on_escalation(move |info| {
    assert!(
      info.stage.hops() >= 1,
      "escalation delivered to custom sink should already have propagated"
    );
    let current = attempts_clone.get();
    attempts_clone.set(current + 1);
    if current == 0 {
      Err(QueueError::Disconnected)
    } else {
      Ok(())
    }
  });

  let panic_flag = Rc::new(Cell::new(true));
  let panic_once = panic_flag.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::System(_) => {}
        Message::User(_) if panic_once.get() => {
          panic_once.set(false);
          panic!("custom escalation failure");
        }
        _ => {}
      },
    )
    .unwrap();

  // consume initial watch message
  block_on(scheduler.dispatch_next()).unwrap();

  actor_ref
    .try_send_with_priority(Message::User(7), DEFAULT_PRIORITY)
    .unwrap();

  // first dispatch: panic occurs and escalation handler fails, causing requeue.
  block_on(scheduler.dispatch_next()).unwrap();
  assert_eq!(attempts.get(), 1);

  // second dispatch: retry succeeds and escalation queue drains.
  block_on(scheduler.dispatch_next()).unwrap();
  assert_eq!(attempts.get(), 2);
  assert!(scheduler.take_escalations().is_empty());
}

#[cfg(feature = "std")]
#[test]
fn scheduler_root_event_listener_broadcasts() {
  use std::sync::{Arc as StdArc, Mutex};

  let runtime = TestMailboxRuntime::unbounded();
  let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
    PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

  let hub = crate::FailureEventHub::new();
  let received: StdArc<Mutex<Vec<FailureInfo>>> = StdArc::new(Mutex::new(Vec::new()));
  let received_clone = received.clone();

  let _subscription = hub.subscribe(Arc::new(move |event| match event {
    crate::FailureEvent::RootEscalated(info) => {
      received_clone.lock().unwrap().push(info.clone());
    }
  }));

  scheduler.set_root_event_listener(Some(hub.listener()));

  let should_panic = Rc::new(Cell::new(true));
  let panic_flag = should_panic.clone();

  let actor_ref = scheduler
    .spawn_actor(
      NoopSupervisor,
      MailboxOptions::default(),
      Arc::new(|sys| Message::System(sys)),
      move |_, msg: Message| match msg {
        Message::System(SystemMessage::Watch(_)) => {}
        Message::User(_) if panic_flag.get() => {
          panic_flag.set(false);
          panic!("hub boom");
        }
        _ => {}
      },
    )
    .unwrap();

  actor_ref
    .try_send_with_priority(Message::User(7), DEFAULT_PRIORITY)
    .unwrap();

  let received_ref = received.clone();
  block_on(scheduler.run_until(|| received_ref.lock().unwrap().is_empty())).unwrap();

  let events = received.lock().unwrap();
  assert_eq!(events.len(), 1);
  assert!(!events[0].reason.is_empty());
}
