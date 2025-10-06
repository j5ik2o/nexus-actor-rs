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
  use crate::actor::supervisor::strategy_restarting::RestartingStrategy;
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

  fn invoke_strategy(
    strategy: RestartingStrategy,
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
  async fn test_handle_child_failure_always_restarts() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = RestartingStrategy::new();

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
  }
}
