extern crate alloc;
extern crate std;

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use std::sync::Arc;

use nexus_actor_core_rs::{ActorId, ActorPath, FailureEvent, FailureInfo, FailureMetadata};
use nexus_actor_core_rs::{ActorSystem, ActorSystemParts, FailureEventStream, MailboxOptions, Props};
use nexus_actor_embedded_rs::{EmbeddedFailureEventHub, ImmediateSpawner, ImmediateTimer, LocalMailboxFactory};

#[test]
fn embedded_actor_runtime_dispatches_message() {
  let components = ActorSystemParts::new(
    LocalMailboxFactory::default(),
    ImmediateSpawner,
    ImmediateTimer,
    EmbeddedFailureEventHub::new(),
  );
  let (mut system, _) = ActorSystem::from_parts(components);

  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
    log_clone.borrow_mut().push(msg);
  });

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn typed actor");

  actor_ref.tell(11).expect("tell message");
  system.run_until_idle().expect("run until idle");

  assert_eq!(log.borrow().as_slice(), &[11]);
}

#[test]
fn embedded_failure_event_hub_broadcasts() {
  let hub = EmbeddedFailureEventHub::new();

  let received = Arc::new(std::sync::Mutex::new(Vec::<FailureEvent>::new()));
  let received_clone = received.clone();

  let _subscription = hub.subscribe(Arc::new(move |event| {
    received_clone.lock().unwrap().push(event);
  }));

  let listener = hub.listener();
  let info = FailureInfo::new_with_metadata(ActorId(1), ActorPath::new(), "boom".into(), FailureMetadata::default());

  listener(FailureEvent::RootEscalated(info.clone()));

  assert_eq!(received.lock().unwrap().len(), 1);
  let guard = received.lock().unwrap();
  let FailureEvent::RootEscalated(recorded) = &guard[0];
  assert_eq!(recorded.actor, info.actor);
}
