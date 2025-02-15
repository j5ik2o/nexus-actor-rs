use async_trait::async_trait;
use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::actor::actor::ErrorReason;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor::RestartStatistics;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
use crate::actor::supervisor::strategy_restarting::RestartingStrategy;
use crate::actor::supervisor::supervision_event::SupervisorEvent;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

#[derive(Clone)]
pub struct Decider(Arc<dyn Fn(ErrorReason) -> BoxFuture<'static, Directive> + Send + Sync + 'static>);

unsafe impl Send for Decider {}
unsafe impl Sync for Decider {}

impl Decider {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ErrorReason) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Directive> + Send + 'static, {
    Decider(Arc::new(move |error| Box::pin(f(error))))
  }

  pub async fn run(&self, reason: ErrorReason) -> Directive {
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
    (self.0.as_ref() as *const dyn Fn(ErrorReason) -> BoxFuture<'static, Directive>).hash(state);
  }
}

#[async_trait]
pub trait SupervisorStrategy: Debug + Send + Sync {
  async fn handle_child_failure(
    &self,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ErrorReason,
    message_handle: MessageHandle,
  );

  fn as_any(&self) -> &dyn std::any::Any;
}

#[async_trait]
pub trait Supervisor: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
  async fn get_children(&self) -> Vec<ExtendedPid>;
  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle);
  async fn restart_children(&self, pids: &[ExtendedPid]);
  async fn stop_children(&self, pids: &[ExtendedPid]);
  async fn resume_children(&self, pids: &[ExtendedPid]);
}

#[derive(Debug, Clone)]
pub struct SupervisorHandle(Arc<Mutex<dyn Supervisor>>);

impl SupervisorHandle {
    pub async fn get_supervisor(&self) -> Arc<Mutex<dyn Supervisor>> {
        self.0.clone()
    }
}

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
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_children().await
  }

  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle) {
    let mg = self.0.lock().await;
    mg.escalate_failure(reason, message_handle).await;
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

pub async fn log_failure(actor_system: ActorSystem, child: &ExtendedPid, reason: ErrorReason, directive: Directive) {
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
  Lazy::new(|| SupervisorStrategyHandle::new(OneForOneStrategy::new(10, Duration::from_secs(10))));

pub static RESTARTING_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> =
  Lazy::new(|| SupervisorStrategyHandle::new(RestartingStrategy::new()));
