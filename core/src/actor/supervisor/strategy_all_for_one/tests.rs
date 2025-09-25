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
  use crate::actor::supervisor::strategy_all_for_one::AllForOneStrategy;
  use crate::actor::supervisor::supervisor_strategy::{Supervisor, SupervisorHandle};
  use crate::actor::supervisor::SupervisorStrategy;
  use crate::generated::actor::Pid;

  #[derive(Debug, Clone)]
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

  #[tokio::test]
  async fn test_handle_child_failure_restart() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Restart });

    // Add multiple children to verify all-for-one behavior
    let child2 = ExtendedPid::new(Pid::new("test", "2"));
    let child3 = ExtendedPid::new(Pid::new("test", "3"));
    {
      let mock_supervisor = supervisor.get_supervisor().await;
      let guard = mock_supervisor.read().await;
      let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    strategy
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
    let mock_supervisor = supervisor.get_supervisor().await;
    let guard = mock_supervisor.read().await;
    let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
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

    let (actor_system, supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Stop });

    // Add multiple children to verify all-for-one behavior
    let child2 = ExtendedPid::new(Pid::new("test", "2"));
    let child3 = ExtendedPid::new(Pid::new("test", "3"));
    {
      let mock_supervisor = supervisor.get_supervisor().await;
      let guard = mock_supervisor.read().await;
      let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    strategy
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
    let mock_supervisor = supervisor.get_supervisor().await;
    let guard = mock_supervisor.read().await;
    let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
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

    let (actor_system, supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Escalate });

    // Add multiple children to verify all-for-one behavior
    let child2 = ExtendedPid::new(Pid::new("test", "2"));
    let child3 = ExtendedPid::new(Pid::new("test", "3"));
    {
      let mock_supervisor = supervisor.get_supervisor().await;
      let guard = mock_supervisor.read().await;
      let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

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
    let mock_supervisor = supervisor.get_supervisor().await;
    let guard = mock_supervisor.read().await;
    let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "escalate");

    // Verify children list is cleared after escalation
    let affected_children = mock_supervisor.children.lock().unwrap().clone();
    assert_eq!(affected_children.len(), 0);
  }

  #[tokio::test]
  #[ignore]
  async fn test_handle_child_failure_resume() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let (actor_system, supervisor, child, rs) = setup_test_environment().await;
    let strategy = AllForOneStrategy::new(3, Duration::from_secs(10))
      .with_decider(|_| async { crate::actor::supervisor::directive::Directive::Resume });

    // Add multiple children to verify all-for-one behavior
    let child2 = ExtendedPid::new(Pid::new("test", "2"));
    let child3 = ExtendedPid::new(Pid::new("test", "3"));
    {
      let mock_supervisor = supervisor.get_supervisor().await;
      let guard = mock_supervisor.read().await;
      let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
      *mock_supervisor.children.lock().unwrap() = vec![child.clone(), child2.clone(), child3.clone()];
    }

    strategy
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
    let mock_supervisor = supervisor.get_supervisor().await;
    let guard = mock_supervisor.read().await;
    let mock_supervisor = guard.as_any().downcast_ref::<MockSupervisor>().unwrap();
    let last_action = mock_supervisor.last_action.lock().unwrap().clone();
    assert_eq!(last_action.as_str(), "resume");

    // Verify all children were affected
    let affected_children = mock_supervisor.children.lock().unwrap().clone();
    assert_eq!(affected_children.len(), 3);
    assert!(affected_children.contains(&child));
    assert!(affected_children.contains(&child2));
    assert!(affected_children.contains(&child3));
  }
}
