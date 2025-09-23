use super::*;
use async_trait::async_trait;
use std::any::Any;
use std::time::Instant;

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

  let resolved = registry
    .get_process(&pid)
    .await
    .expect("プロセスが存在するはず");

  assert_eq!(resolved, first_handle, "同名登録時にも既存プロセスを維持すべき");
  assert_ne!(resolved, second_handle, "新しいプロセスで上書きされてはならない");
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
