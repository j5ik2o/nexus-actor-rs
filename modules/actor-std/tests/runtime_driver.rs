use std::sync::{Arc, Mutex};

use nexus_actor_core_rs::{ActorSystem, MailboxOptions, Props, RuntimeComponents};
use nexus_actor_std_rs::{FailureEventHub, TokioMailboxRuntime, TokioSpawner, TokioSystemHandle, TokioTimer};

async fn run_tokio_actor_runtime_processes_messages() {
  let components = RuntimeComponents::new(TokioMailboxRuntime, TokioSpawner, TokioTimer, FailureEventHub::new());
  let (mut system, _) = ActorSystem::from_runtime_components(components);

  let state: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
  let state_clone = state.clone();

  let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
    state_clone.lock().unwrap().push(msg);
  });

  let mut root = system.root_context();
  let actor_ref = root.spawn(props).expect("spawn typed actor");

  actor_ref.tell(7).expect("tell");
  system.run_until_idle().expect("run until idle");

  assert_eq!(state.lock().unwrap().as_slice(), &[7]);
}

#[tokio::test(flavor = "current_thread")]
async fn tokio_actor_runtime_processes_messages() {
  run_tokio_actor_runtime_processes_messages().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_actor_runtime_processes_messages_multi_thread() {
  run_tokio_actor_runtime_processes_messages().await;
}

async fn run_tokio_system_handle_can_be_aborted() {
  tokio::task::LocalSet::new()
    .run_until(async move {
      let components = RuntimeComponents::new(TokioMailboxRuntime, TokioSpawner, TokioTimer, FailureEventHub::new());
      let (system, _) = ActorSystem::from_runtime_components(components);
      let handle: TokioSystemHandle<u32> = TokioSystemHandle::start_local(system.into_runner());
      let listener = handle.spawn_ctrl_c_listener();
      handle.abort();
      listener.abort();
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn tokio_system_handle_can_be_aborted() {
  run_tokio_system_handle_can_be_aborted().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_system_handle_can_be_aborted_multi_thread() {
  run_tokio_system_handle_can_be_aborted().await;
}
