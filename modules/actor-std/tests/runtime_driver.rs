use std::sync::{Arc, Mutex};

use nexus_actor_core_rs::{MailboxOptions, Props};

use nexus_actor_std_rs::TokioActorRuntime;

#[tokio::test(flavor = "current_thread")]
async fn tokio_actor_runtime_processes_messages() {
  let mut runtime: TokioActorRuntime<u32> = TokioActorRuntime::new();

  let state: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
  let state_clone = state.clone();

  let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
    state_clone.lock().unwrap().push(msg);
  });

  let actor_ref = runtime.spawn_actor(props).expect("spawn typed actor");

  actor_ref.tell(7).expect("tell");
  runtime.dispatch_next().await.expect("dispatch next");

  assert_eq!(state.lock().unwrap().as_slice(), &[7]);
}
