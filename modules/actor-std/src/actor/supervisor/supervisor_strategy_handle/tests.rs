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
  use crate::actor::supervisor::core_adapters::{StdSupervisorAdapter, StdSupervisorContext};
  use crate::actor::supervisor::supervisor_strategy::Supervisor;
  use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
  use crate::actor::supervisor::SupervisorHandle;
  use nexus_actor_core_rs::actor::core_types::pid::CorePid;
  use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
  use nexus_actor_core_rs::supervisor::{
    CoreSupervisor, CoreSupervisorContext, CoreSupervisorStrategy, CoreSupervisorStrategyFuture,
  };

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
  impl crate::actor::supervisor::supervisor_strategy::Supervisor for MockSupervisor {
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
    let supervisor_arc: Arc<dyn crate::actor::supervisor::supervisor_strategy::Supervisor> =
      supervisor_instance.clone();
    let supervisor = SupervisorHandle::new_arc(supervisor_arc);
    let child = CorePid::new("test", "1");
    let rs = RestartStatistics::with_runtime(&actor_system.core_runtime());
    (actor_system, supervisor, supervisor_instance, child, rs)
  }

  #[derive(Debug, Clone)]
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

  impl CoreSupervisorStrategy for MockStrategy {
    fn handle_child_failure<'a>(
      &'a self,
      _ctx: &'a dyn CoreSupervisorContext,
      supervisor: &'a dyn CoreSupervisor,
      child: CorePid,
      tracker: &'a mut CoreRestartTracker,
      _reason: nexus_actor_core_rs::error::ErrorReasonCore,
      _message: MessageHandle,
    ) -> CoreSupervisorStrategyFuture<'a> {
      let last_action = self.last_action.clone();
      let supervisor_adapter = supervisor
        .as_any()
        .downcast_ref::<StdSupervisorAdapter>()
        .expect("StdSupervisorAdapter expected");
      let supervisor_handle = supervisor_adapter.handle();
      Box::pin(async move {
        let children = [child.clone()];
        supervisor_handle.restart_children(&children).await;
        *last_action.lock().unwrap() = format!("handle_failure_{}", child.id());
        // tracker remains unchanged
        let cloned = tracker.clone();
        *tracker = cloned;
      })
    }
  }

  #[tokio::test]
  async fn test_handle_child_failure_delegates_to_strategy() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, mock_supervisor, child, rs) = setup_test_environment().await;
    let strategy = MockStrategy::new();
    let last_action = strategy.last_action.clone();
    let strategy_handle = SupervisorStrategyHandle::new(strategy);

    let core_context = StdSupervisorContext::new(actor_system.clone());
    let core_supervisor = supervisor.core_adapter();
    let mut tracker = rs.to_core_tracker().await;

    strategy_handle
      .core_strategy()
      .handle_child_failure(
        &core_context,
        &core_supervisor,
        child.clone(),
        &mut tracker,
        ErrorReason::new("test", 1).as_core().clone(),
        MessageHandle::new(String::from("test")),
      )
      .await;

    rs.overwrite_with(tracker).await;

    tokio::time::sleep(Duration::from_millis(50)).await;
    let action = last_action.lock().unwrap().clone();
    assert_eq!(action, format!("handle_failure_{}", child.id()));

    let supervisor_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(supervisor_action.as_str(), "restart");
  }
}
