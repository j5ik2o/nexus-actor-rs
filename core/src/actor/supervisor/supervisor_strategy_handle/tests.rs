#[cfg(test)]
mod test {
  use std::env;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  use async_trait::async_trait;
  use tracing_subscriber::EnvFilter;

  use crate::actor::actor_system::ActorSystem;
  use crate::actor::core::{ErrorReason, ExtendedPid, RestartStatistics};
  use crate::actor::message::MessageHandle;
  use crate::actor::supervisor::supervisor_strategy::{Supervisor, SupervisorStrategy};
  use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
  use crate::actor::supervisor::SupervisorHandle;
  use crate::generated::actor::Pid;

  #[derive(Debug)]
  struct MockSupervisor {
    last_action: Arc<Mutex<String>>,
    children: Arc<Mutex<Vec<ExtendedPid>>>,
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

    async fn get_children(&self) -> Vec<ExtendedPid> {
      self.children.lock().unwrap().clone()
    }

    async fn resume_children(&self, children: &[ExtendedPid]) {
      *self.last_action.lock().unwrap() = "resume".to_string();
      *self.children.lock().unwrap() = children.to_vec();
    }

    async fn restart_children(&self, children: &[ExtendedPid]) {
      *self.last_action.lock().unwrap() = "restart".to_string();
      *self.children.lock().unwrap() = children.to_vec();
    }

    async fn stop_children(&self, children: &[ExtendedPid]) {
      *self.last_action.lock().unwrap() = "stop".to_string();
      *self.children.lock().unwrap() = children.to_vec();
    }

    async fn escalate_failure(&self, _: ErrorReason, _: MessageHandle) {
      *self.last_action.lock().unwrap() = "escalate".to_string();
      self.children.lock().unwrap().clear();
    }
  }

  async fn setup_test_environment() -> (ActorSystem, SupervisorHandle, ExtendedPid, RestartStatistics) {
    let actor_system = ActorSystem::new().await.unwrap();
    let supervisor = SupervisorHandle::new(MockSupervisor::new());
    let child = ExtendedPid::new(Pid::new("test", "1"));
    let rs = RestartStatistics::new();
    (actor_system, supervisor, child, rs)
  }

  #[derive(Debug)]
  struct MockStrategy {
    last_action: Arc<Mutex<String>>,
  }

  impl MockStrategy {
    fn new() -> Self {
      Self {
        last_action: Arc::new(Mutex::new(String::new())),
      }
    }
  }

  #[async_trait]
  impl SupervisorStrategy for MockStrategy {
    async fn handle_child_failure(
      &self,
      _: ActorSystem,
      supervisor: SupervisorHandle,
      child: ExtendedPid,
      _: RestartStatistics,
      _: ErrorReason,
      _: MessageHandle,
    ) {
      *self.last_action.lock().unwrap() = format!("handle_failure_{}", child.id());
      supervisor.restart_children(&[child]).await;
    }

    fn as_any(&self) -> &dyn std::any::Any {
      self
    }
  }

  #[tokio::test]
  async fn test_handle_child_failure_delegates_to_strategy() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, child, rs) = setup_test_environment().await;
    let mock_strategy = MockStrategy::new();
    let last_action = mock_strategy.last_action.clone();
    let strategy_handle = SupervisorStrategyHandle::new(mock_strategy);

    strategy_handle
      .handle_child_failure(
        actor_system,
        supervisor.clone(),
        child.clone(),
        rs,
        ErrorReason::new("test", 1),
        MessageHandle::new(String::from("test")),
      )
      .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let action = last_action.lock().unwrap().clone();
    assert_eq!(action, format!("handle_failure_{}", child.id()));

    // Verify supervisor action was called through delegation
    let mock_supervisor = supervisor.get_supervisor().await;
    let guard = mock_supervisor.lock().await;
    let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
    let supervisor_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(supervisor_action.as_str(), "restart");
  }
}
