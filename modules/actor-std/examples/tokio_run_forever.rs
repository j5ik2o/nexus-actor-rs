//! Tokio 上で `ActorSystem::run_until` を起動する最小サンプル。

use nexus_actor_core_rs::{ActorSystem, MailboxOptions, Props};
use nexus_actor_std_rs::TokioMailboxFactory;
use std::sync::{Arc, Mutex};

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let runtime = TokioMailboxFactory;
  let mut system: ActorSystem<u32, _> = ActorSystem::new(runtime);
  let mut root = system.root_context();

  let log = Arc::new(Mutex::new(Vec::new()));
  let log_clone = log.clone();

  let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
    log_clone.lock().unwrap().push(msg);
  });

  let actor_ref = root.spawn(props).expect("spawn actor");

  actor_ref.tell(42).expect("tell");

  let mut iterations = 0usize;
  system
    .run_until(move || {
      if iterations == 0 {
        iterations += 1;
        true
      } else {
        false
      }
    })
    .await
    .expect("run until");

  assert_eq!(*log.lock().unwrap(), vec![42]);
}
