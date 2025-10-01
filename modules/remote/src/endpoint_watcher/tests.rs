use super::*;
use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::remote::Remote;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::generated::actor::Pid;
use std::sync::Arc;
use tokio::sync::OnceCell;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

async fn setup_remote_for_tests() -> TestResult<Arc<Remote>> {
  static INIT: OnceCell<()> = OnceCell::const_new();
  let _ = INIT
    .get_or_init(|| async {
      let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    })
    .await;

  let system = ActorSystem::new().await?;
  let config = Config::from(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19580),
  ])
  .await;
  let remote = Remote::new(system, config).await;
  Ok(Arc::new(remote))
}

#[tokio::test]
async fn get_address_and_watched_return_expected_values() -> TestResult<()> {
  let remote = setup_remote_for_tests().await?;
  let watcher = EndpointWatcher::new(Arc::downgrade(&remote), "endpoint-test".to_string());

  assert_eq!(watcher.get_address(), "endpoint-test");

  let watched = watcher.get_watched();
  assert!(watched.is_empty());

  Ok(())
}

#[tokio::test]
async fn get_watched_exposes_live_pid_set_map() -> TestResult<()> {
  let remote = setup_remote_for_tests().await?;
  let watcher = EndpointWatcher::new(Arc::downgrade(&remote), "endpoint-test".to_string());

  let target_pid = Pid {
    address: "remote-address".to_string(),
    id: "watched".to_string(),
    request_id: 0,
  };
  watcher.add_watch_pid("watcher", target_pid.clone()).await;

  let stored_map = watcher.get_watched();
  let entry = stored_map.get("watcher").expect("watcher entry missing");
  assert!(entry.contains(&target_pid).await);

  Ok(())
}

#[tokio::test]
async fn add_and_remove_watch_pid_updates_pid_set_lifecycle() -> TestResult<()> {
  let remote = setup_remote_for_tests().await?;
  let watcher = EndpointWatcher::new(Arc::downgrade(&remote), "endpoint-test".to_string());
  let watcher_id = "watcher";

  let pid_a = Pid {
    address: "remote-address".to_string(),
    id: "watched-a".to_string(),
    request_id: 1,
  };
  let pid_b = Pid {
    address: "remote-address".to_string(),
    id: "watched-b".to_string(),
    request_id: 2,
  };

  watcher.add_watch_pid(watcher_id, pid_a.clone()).await;
  watcher.add_watch_pid(watcher_id, pid_b.clone()).await;

  let active_set = watcher
    .get_pid_set(watcher_id)
    .expect("pid set must exist after adding watchers");
  assert_eq!(active_set.len().await, 2);
  assert!(active_set.contains(&pid_a).await);
  assert!(active_set.contains(&pid_b).await);
  assert!(!watcher.prune_if_empty(watcher_id).await);

  assert!(watcher.remove_watch_pid(watcher_id, &pid_a).await);
  let remaining = watcher
    .get_pid_set(watcher_id)
    .expect("pid set must remain while entries still exist");
  assert_eq!(remaining.len().await, 1);
  assert!(!remaining.contains(&pid_a).await);
  assert!(remaining.contains(&pid_b).await);

  assert!(watcher.remove_watch_pid(watcher_id, &pid_b).await);
  assert!(watcher.get_pid_set(watcher_id).is_none());
  assert!(watcher.watched_snapshot().is_empty());
  assert!(!watcher.remove_watch_pid(watcher_id, &pid_b).await);
  assert!(!watcher.prune_if_empty(watcher_id).await);

  Ok(())
}
