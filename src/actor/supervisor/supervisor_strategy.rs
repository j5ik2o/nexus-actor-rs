use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
use crate::actor::supervisor::strategy_restarting::RestartingStrategy;
use crate::actor::supervisor::supervision_event::SupervisorEvent;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
use async_trait::async_trait;
use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Decider(Arc<dyn Fn(ActorInnerError) -> BoxFuture<'static, Directive> + Send + Sync>);

impl Decider {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ActorInnerError) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Directive> + Send + 'static, {
    Decider(Arc::new(move |error| Box::pin(f(error))))
  }

  pub async fn run(&self, reason: ActorInnerError) -> Directive {
    (self.0)(reason).await
  }
}

impl Debug for Decider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "DeciderFunc")
  }
}

impl PartialEq for Decider {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for Decider {}

impl std::hash::Hash for Decider {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ActorInnerError) -> BoxFuture<'static, Directive>).hash(state);
  }
}

#[async_trait]
pub trait SupervisorStrategy: Debug + Send + Sync {
  async fn handle_child_failure(
    &self,
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  );

  fn as_any(&self) -> &dyn std::any::Any;
}

#[async_trait]
pub trait Supervisor: Debug + Send + Sync + 'static {
  async fn get_children(&self) -> Vec<ExtendedPid>;
  async fn escalate_failure(&self, reason: ActorInnerError, message: MessageHandle);
  async fn restart_children(&self, pids: &[ExtendedPid]);
  async fn stop_children(&self, pids: &[ExtendedPid]);
  async fn resume_children(&self, pids: &[ExtendedPid]);
}

#[derive(Debug, Clone)]
pub struct SupervisorHandle(Arc<Mutex<dyn Supervisor>>);

impl SupervisorHandle {
  pub fn new_arc(s: Arc<Mutex<dyn Supervisor>>) -> Self {
    SupervisorHandle(s)
  }

  pub fn new(s: impl Supervisor + 'static) -> Self {
    SupervisorHandle(Arc::new(Mutex::new(s)))
  }
}

impl PartialEq for SupervisorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SupervisorHandle {}

impl std::hash::Hash for SupervisorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn Supervisor>).hash(state);
  }
}

#[async_trait]
impl Supervisor for SupervisorHandle {
  async fn get_children(&self) -> Vec<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_children().await
  }

  async fn escalate_failure(&self, reason: ActorInnerError, message: MessageHandle) {
    let mg = self.0.lock().await;
    mg.escalate_failure(reason, message).await;
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    let mg = self.0.lock().await;
    mg.restart_children(pids).await;
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    let mg = self.0.lock().await;
    mg.stop_children(pids).await;
  }

  async fn resume_children(&self, pids: &[ExtendedPid]) {
    let mg = self.0.lock().await;
    mg.resume_children(pids).await;
  }
}

pub async fn log_failure(
  actor_system: &ActorSystem,
  child: &ExtendedPid,
  reason: ActorInnerError,
  directive: Directive,
) {
  actor_system
    .get_event_stream()
    .await
    .publish(MessageHandle::new(SupervisorEvent {
      child: child.clone(),
      reason,
      directive,
    }))
    .await;
}

pub static DEFAULT_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> =
  Lazy::new(|| SupervisorStrategyHandle::new(OneForOneStrategy::new(10, tokio::time::Duration::from_secs(10))));

pub static RESTARTING_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> =
  Lazy::new(|| SupervisorStrategyHandle::new(RestartingStrategy::new()));
