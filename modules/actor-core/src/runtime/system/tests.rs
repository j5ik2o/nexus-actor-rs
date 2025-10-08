#![allow(deprecated, unused_imports)]

use super::*;
use crate::runtime::guardian::AlwaysRestart;
use crate::runtime::mailbox::test_support::TestMailboxFactory;
use crate::{MailboxOptions, SystemMessage};
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefCell;

#[cfg(feature = "std")]
use futures::executor::block_on;
use nexus_utils_core_rs::{Element, DEFAULT_PRIORITY};

#[cfg(feature = "std")]
#[derive(Debug, Clone)]
enum Message {
  User(u32),
  System,
}

#[cfg(feature = "std")]
impl Element for Message {}

#[cfg(feature = "std")]
#[test]
fn actor_system_spawns_and_processes_messages() {
  let factory = TestMailboxFactory::unbounded();
  let mut system: InternalActorSystem<Message, _, AlwaysRestart> = InternalActorSystem::new(factory);

  let map_system = Arc::new(|_: SystemMessage| Message::System);
  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let mut root = system.root_context();
  let actor_ref = root
    .spawn(InternalProps::new(
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

  block_on(root.dispatch_next()).expect("dispatch");

  assert_eq!(log.borrow().as_slice(), &[7]);
}
