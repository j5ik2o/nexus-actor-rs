use super::*;
use async_trait::async_trait;
use std::any::Any;
use std::time::{Duration, Instant};

use tokio::time::timeout;

use crate::actor::message::MessageHandle;

const ITERATIONS: u32 = 1_000_000; // 適切な反復回数に調整してください

#[test]
fn test_uint64_to_id() {
  let start = Instant::now();
  let mut s = String::new();
  for i in 0..ITERATIONS {
    s = uint64_to_id(u64::from(i) << 5);
  }
  let duration = start.elapsed();
  tracing::debug!("uint64_to_id: {:?}, last result: {}", duration, s);
}

#[tokio::test]
async fn test_add_process_replaces_existing_entry_on_name_collision() {
  let system = ActorSystem::new().await.expect("actor system init");
  let registry = system.get_process_registry().await;

  let first_handle = ProcessHandle::new(DummyProcess::new("first"));
  let (pid, inserted_first) = registry.add_process(first_handle.clone(), "dup").await;
  assert!(inserted_first, "初回登録は成功する想定");

  let second_handle = ProcessHandle::new(DummyProcess::new("second"));
  let (_pid2, inserted_second) = registry.add_process(second_handle.clone(), "dup").await;
  assert!(!inserted_second, "重複検知は動作しているが、既存プロセスを保持すべき");

  let resolved = registry.get_process(&pid).await.expect("プロセスが存在するはず");

  assert_eq!(resolved, first_handle, "同名登録時にも既存プロセスを維持すべき");
  assert_ne!(resolved, second_handle, "新しいプロセスで上書きされてはならない");
}

#[tokio::test]
async fn test_list_local_pids_returns_registered_ids() {
  let system = ActorSystem::new().await.expect("actor system init");
  let registry = system.get_process_registry().await;

  let _ = registry
    .add_process(ProcessHandle::new(DummyProcess::new("first")), "proc_a")
    .await;
  let _ = registry
    .add_process(ProcessHandle::new(DummyProcess::new("second")), "proc_b")
    .await;

  let mut listed = registry.list_local_pids().await;
  listed.sort_by(|a, b| a.id.cmp(&b.id));

  let ids: Vec<_> = listed.into_iter().map(|pid| pid.id).collect();
  assert!(ids.contains(&"proc_a".to_string()));
  assert!(ids.contains(&"proc_b".to_string()));
}

#[tokio::test]
async fn test_find_local_process_handle() {
  let system = ActorSystem::new().await.expect("actor system init");
  let registry = system.get_process_registry().await;

  let (_pid, _) = registry
    .add_process(ProcessHandle::new(DummyProcess::new("first")), "proc_a")
    .await;

  let found = registry.find_local_process_handle("proc_a").await;
  assert!(found.is_some(), "既存プロセスが取得できること");

  let missing = registry.find_local_process_handle("missing").await;
  assert!(missing.is_none(), "存在しないプロセスは None を返す");
}

#[tokio::test]
async fn test_get_process_remote_handler_reentrancy() {
  let system = ActorSystem::new().await.expect("actor system init");
  let registry = system.get_process_registry().await;

  let registry_for_registration = registry.clone();
  let registry_for_handler = registry.clone();

  registry_for_registration.register_address_resolver(AddressResolver::new(move |_core_pid| {
    let registry_for_handler = registry_for_handler.clone();
    async move {
      let registry_to_modify = registry_for_handler.clone();
      registry_to_modify.register_address_resolver(AddressResolver::new(|_| async { None }));
      None
    }
  }));

  let remote_pid = ExtendedPid::new(Pid::new("remote-host", "pid"));
  let result = timeout(Duration::from_millis(200), registry.get_process(&remote_pid)).await;

  assert!(result.is_ok(), "リモートハンドラ内の再登録でデッドロックしないこと");
  assert!(result.unwrap().is_some(), "処理後はデッドレタープロセスが返る想定");
}

#[derive(Debug, Clone)]
struct DummyProcess {
  name: &'static str,
}

impl DummyProcess {
  fn new(name: &'static str) -> Self {
    Self { name }
  }
}

#[async_trait]
impl Process for DummyProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, _: MessageHandle) {}

  async fn send_system_message(&self, _: &ExtendedPid, _: MessageHandle) {}

  async fn stop(&self, _: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}
