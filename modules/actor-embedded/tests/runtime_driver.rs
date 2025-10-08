extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use futures::executor::block_on;

use nexus_actor_core_rs::{MailboxOptions, Props};
use nexus_actor_embedded_rs::EmbeddedActorRuntime;

#[test]
fn embedded_actor_runtime_dispatches_message() {
  let mut runtime: EmbeddedActorRuntime<u32> = EmbeddedActorRuntime::new();

  let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
  let log_clone = log.clone();

  let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
    log_clone.borrow_mut().push(msg);
  });

  let actor_ref = runtime.spawn_actor(props).expect("spawn typed actor");

  actor_ref.tell(11).expect("tell message");

  block_on(async {
    runtime.dispatch_next().await.expect("dispatch next");
  });

  assert_eq!(log.borrow().as_slice(), &[11]);
}
