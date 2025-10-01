#[cfg(test)]
mod test {
  use std::any::Any;
  use std::env;
  use std::sync::{Arc, Mutex};
  use std::time::{Duration, Instant};

  use async_trait::async_trait;
  use tracing_subscriber::EnvFilter;

  use crate::actor::actor_system::ActorSystem;
  use crate::actor::core::{ErrorReason, RestartStatistics};
  use crate::actor::message::MessageHandle;
  use crate::actor::supervisor::directive::Directive;
  use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
  use crate::actor::supervisor::supervisor_strategy::{Supervisor, SupervisorHandle, SupervisorStrategy};
  use nexus_actor_core_rs::actor::core_types::pid::CorePid;

  #[tokio::test]
  async fn test_one_for_one_strategy_request_restart_permission() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let cases = vec![
      (
        "no restart if max retries is 0",
        OneForOneStrategy::new(0, Duration::from_secs(0)),
        RestartStatistics::new(),
        true,
        0,
      ),
      (
        "restart when duration is 0",
        OneForOneStrategy::new(1, Duration::from_secs(0)),
        RestartStatistics::new(),
        false,
        1,
      ),
      (
        "no restart when duration is 0 and exceeds max retries",
        OneForOneStrategy::new(1, Duration::from_secs(0)),
        RestartStatistics::with_values(vec![Instant::now() - Duration::from_secs(1)]),
        true,
        0,
      ),
      (
        "restart when duration set and within window",
        OneForOneStrategy::new(2, Duration::from_secs(10)),
        RestartStatistics::with_values(vec![Instant::now() - Duration::from_secs(5)]),
        false,
        2,
      ),
      (
        "no restart when duration set, within window and exceeds max retries",
        OneForOneStrategy::new(1, Duration::from_secs(10)),
        RestartStatistics::with_values(vec![
          Instant::now() - Duration::from_secs(5),
          Instant::now() - Duration::from_secs(5),
        ]),
        true,
        0,
      ),
      (
        "restart and FailureCount reset when duration set and outside window",
        OneForOneStrategy::new(1, Duration::from_secs(10)),
        RestartStatistics::with_values(vec![
          Instant::now() - Duration::from_secs(11),
          Instant::now() - Duration::from_secs(11),
        ]),
        false,
        1,
      ),
    ];

    for (name, s, mut rs, expected_result, expected_count) in cases {
      let actual = s.should_stop(&mut rs).await;
      assert_eq!(actual, expected_result, "{}", name);
      assert_eq!(
        rs.number_of_failures(s.within_duration).await,
        expected_count,
        "{}",
        name
      );
    }
  }

  #[derive(Debug, Clone)]
  struct MockSupervisor {
    last_action: Arc<Mutex<String>>,
    children: Arc<Mutex<Vec<CorePid>>>,
  }

  impl MockSupervisor {
    fn new() -> Self {
      Self {
        last_action: Arc::new(Mutex::new(String::new())),
        children: Arc::new(Mutex::new(Vec::new())),
      }
    }
  }

  #[async_trait]
  impl Supervisor for MockSupervisor {
    fn as_any(&self) -> &dyn Any {
      self
    }

    async fn get_children(&self) -> Vec<CorePid> {
      self.children.lock().unwrap().clone()
    }

    async fn resume_children(&self, children: &[CorePid]) {
      *self.last_action.lock().unwrap() = "resume".to_string();
      *self.children.lock().unwrap() = children.to_vec();
    }

    async fn restart_children(&self, children: &[CorePid]) {
      *self.last_action.lock().unwrap() = "restart".to_string();
      *self.children.lock().unwrap() = children.to_vec();
    }

    async fn stop_children(&self, children: &[CorePid]) {
      *self.last_action.lock().unwrap() = "stop".to_string();
      *self.children.lock().unwrap() = children.to_vec();
    }

    async fn escalate_failure(&self, _: ErrorReason, _: MessageHandle) {
      *self.last_action.lock().unwrap() = "escalate".to_string();
      self.children.lock().unwrap().clear();
    }
  }

  async fn setup_test_environment() -> (
    ActorSystem,
    SupervisorHandle,
    Arc<MockSupervisor>,
    CorePid,
    RestartStatistics,
  ) {
    let actor_system = ActorSystem::new().await.unwrap();
    let supervisor_instance = Arc::new(MockSupervisor::new());
    let supervisor_arc: Arc<dyn Supervisor> = supervisor_instance.clone();
    let supervisor = SupervisorHandle::new_arc(supervisor_arc);
    let child = CorePid::new("test", "1");
    let rs = RestartStatistics::new();
    (actor_system, supervisor, supervisor_instance, child, rs)
  }

  #[tokio::test]
  async fn test_handle_child_failure_resume() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = OneForOneStrategy::new(3, Duration::from_secs(10)).with_decider(|_| async { Directive::Resume });

    strategy
      .handle_child_failure(
        actor_system,
        supervisor.clone(),
        child,
        rs,
        ErrorReason::new("test", 1),
        MessageHandle::new(String::from("test")),
      )
      .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "resume");
  }

  #[tokio::test]
  async fn test_handle_child_failure_restart() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = OneForOneStrategy::new(3, Duration::from_secs(10)).with_decider(|_| async { Directive::Restart });

    strategy
      .handle_child_failure(
        actor_system,
        supervisor.clone(),
        child,
        rs,
        ErrorReason::new("test", 1),
        MessageHandle::new(String::from("test")),
      )
      .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "restart");
  }

  #[tokio::test]
  async fn test_handle_child_failure_stop() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = OneForOneStrategy::new(3, Duration::from_secs(10)).with_decider(|_| async { Directive::Stop });

    strategy
      .handle_child_failure(
        actor_system,
        supervisor.clone(),
        child,
        rs,
        ErrorReason::new("test", 1),
        MessageHandle::new(String::from("test")),
      )
      .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "stop");
  }

  #[tokio::test]
  async fn test_handle_child_failure_escalate() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = OneForOneStrategy::new(3, Duration::from_secs(10)).with_decider(|_| async { Directive::Escalate });

    strategy
      .handle_child_failure(
        actor_system,
        supervisor.clone(),
        child,
        rs,
        ErrorReason::new("test", 1),
        MessageHandle::new(String::from("test")),
      )
      .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "escalate");
  }
}
