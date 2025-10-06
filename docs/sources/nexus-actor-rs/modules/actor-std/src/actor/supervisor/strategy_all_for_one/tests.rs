#[cfg(test)]
mod test {
  use std::env;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  use async_trait::async_trait;
  use tracing_subscriber::EnvFilter;

  use crate::actor::actor_system::ActorSystem;
  use crate::actor::core::{ErrorReason, RestartStatistics};
  use crate::actor::message::MessageHandle;
  use crate::actor::supervisor::core_adapters::StdSupervisorContext;
  use crate::actor::supervisor::strategy_all_for_one::AllForOneStrategy;
  use crate::actor::supervisor::supervisor_strategy::{Supervisor, SupervisorHandle};
  use crate::actor::supervisor::SupervisorStrategyHandle;
  use nexus_actor_core_rs::actor::core_types::pid::CorePid;
  use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;

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
    fn as_any(&self) -> &dyn std::any::Any {
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
    let rs = RestartStatistics::with_runtime(&actor_system.core_runtime());
    (actor_system, supervisor, supervisor_instance, child, rs)
  }

  fn invoke_strategy<'a>(
    strategy: AllForOneStrategy,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: CorePid,
    mut tracker: CoreRestartTracker,
    reason: ErrorReason,
    message_handle: MessageHandle,
  ) -> impl std::future::Future<Output = CoreRestartTracker> {
    async move {
      let handle = SupervisorStrategyHandle::new(strategy);
      let core_context = StdSupervisorContext::new(actor_system.clone());
      let core_supervisor = supervisor.core_adapter();
      handle
        .core_strategy()
        .handle_child_failure(
          &core_context,
          &core_supervisor,
          child,
          &mut tracker,
          reason.as_core().clone(),
          message_handle,
        )
        .await;
      tracker
    }
  }

  #[tokio::test]
  async fn should_stop_respects_core_runtime_clock() {
    let strategy = AllForOneStrategy::new(1, Duration::from_millis(40));
    let mut stats = RestartStatistics::with_runtime(&crate::runtime::tokio_core_runtime());

    let first = strategy.should_stop(&mut stats).await;
    assert!(!first, "first failure is within restart tolerance");

    tokio::time::sleep(Duration::from_millis(10)).await;

    let second = strategy.should_stop(&mut stats).await;
    assert!(second, "consecutive failures within time window trigger stop");

    tokio::time::sleep(Duration::from_millis(60)).await;

    let third = strategy.should_stop(&mut stats).await;
    assert!(!third, "restart budget resets after time window expires");
  }

  #[tokio::test]
  async fn test_handle_child_failure_restart() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Restart });

    // Add multiple children to verify all-for-one behavior
    let child2 = CorePid::new("test", "2");
    let child3 = CorePid::new("test", "3");
    {
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    invoke_strategy(
      strategy,
      actor_system,
      supervisor.clone(),
      child.clone(),
      rs.to_core_tracker().await,
      ErrorReason::new("test", 1),
      MessageHandle::new(String::from("test")),
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "restart");

    // Verify all children were affected
    let affected_children = mock_supervisor.children.lock().unwrap().clone();
    assert_eq!(affected_children.len(), 3);
    assert!(affected_children.contains(&child));
    assert!(affected_children.contains(&child2));
    assert!(affected_children.contains(&child3));
  }

  #[tokio::test]
  async fn test_handle_child_failure_stop() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Stop });

    // Add multiple children to verify all-for-one behavior
    let child2 = CorePid::new("test", "2");
    let child3 = CorePid::new("test", "3");
    {
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    invoke_strategy(
      strategy,
      actor_system,
      supervisor.clone(),
      child.clone(),
      rs.to_core_tracker().await,
      ErrorReason::new("test", 1),
      MessageHandle::new(String::from("test")),
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "stop");

    // Verify all children were affected
    let affected_children = mock_supervisor.children.lock().unwrap().clone();
    assert_eq!(affected_children.len(), 3);
    assert!(affected_children.contains(&child));
    assert!(affected_children.contains(&child2));
    assert!(affected_children.contains(&child3));
  }

  #[tokio::test]
  async fn test_handle_child_failure_escalate() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Escalate });

    // Add multiple children to verify all-for-one behavior
    let child2 = CorePid::new("test", "2");
    let child3 = CorePid::new("test", "3");
    {
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    invoke_strategy(
      strategy,
      actor_system,
      supervisor.clone(),
      child,
      rs.to_core_tracker().await,
      ErrorReason::new("test", 1),
      MessageHandle::new(String::from("test")),
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "escalate");

    // Verify children list is cleared after escalation
    let affected_children = mock_supervisor.children.lock().unwrap().clone();
    assert_eq!(affected_children.len(), 0);
  }

  #[tokio::test]
  async fn test_handle_child_failure_resume() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Resume });

    // Add multiple children to verify all-for-one behavior
    let child2 = CorePid::new("test", "2");
    let child3 = CorePid::new("test", "3");
    {
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    invoke_strategy(
      strategy,
      actor_system,
      supervisor.clone(),
      child.clone(),
      rs.to_core_tracker().await,
      ErrorReason::new("test", 1),
      MessageHandle::new(String::from("test")),
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "resume");

    // Verify resume targetsのみ故障した子のみ
    let affected_children = mock_supervisor.children.lock().unwrap().clone();
    assert_eq!(affected_children.len(), 1);
    assert_eq!(affected_children[0], child);
  }
}
