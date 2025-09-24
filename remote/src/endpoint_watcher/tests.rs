use super::*;
use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::remote::Remote;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::core::PidSet;
use nexus_actor_core_rs::generated::actor::Pid;
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

  let watched_map = watcher.get_watched();
  let mut pid_set = PidSet::new().await;
  let target_pid = Pid {
    address: "remote-address".to_string(),
    id: "watched".to_string(),
    request_id: 0,
  };
  pid_set.add(target_pid.clone()).await;
  watched_map.insert("watcher".to_string(), pid_set);

  let stored_map = watcher.get_watched();
  let entry = stored_map.get("watcher").expect("watcher entry missing");
  assert!(entry.contains(&target_pid).await);

  Ok(())
}
