use std::sync::{Arc, Mutex};

use nexus_actor_core_rs::{MailboxOptions, Props};
use nexus_actor_std_rs::{TokioActorRuntime, TokioSystemHandle};

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
  runtime.run_until_idle().expect("run until idle");

  assert_eq!(state.lock().unwrap().as_slice(), &[7]);
}

#[tokio::test(flavor = "current_thread")]
async fn tokio_system_handle_can_be_aborted() {
  tokio::task::LocalSet::new()
    .run_until(async move {
      let runtime: TokioActorRuntime<u32> = TokioActorRuntime::new();
      let handle: TokioSystemHandle<u32> = runtime.start_local();
      let listener = handle.spawn_ctrl_c_listener();
      handle.abort();
      listener.abort();
    })
    .await;
}
