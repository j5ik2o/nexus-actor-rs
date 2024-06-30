use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor::ActorInnerError;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::strategy_on_for_one::OneForOneStrategy;
use crate::actor::supervisor::strategy_restarting::RestartingStrategy;
use crate::actor::supervisor::supervision_event::SupervisorEvent;

#[derive(Clone)]
pub struct DeciderFunc(Arc<dyn Fn(ActorInnerError) -> Directive + Send + Sync>);

impl Debug for DeciderFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "DeciderFunc")
  }
}

impl PartialEq for DeciderFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for DeciderFunc {}

impl std::hash::Hash for DeciderFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ActorInnerError) -> Directive).hash(state);
  }
}

impl DeciderFunc {
  pub fn new(f: impl Fn(ActorInnerError) -> Directive + Send + Sync + 'static) -> Self {
    DeciderFunc(Arc::new(f))
  }

  pub fn run(&self, reason: ActorInnerError) -> Directive {
    (self.0)(reason)
  }
}

#[async_trait]
pub trait SupervisorStrategy: Debug + Send + Sync {
  async fn handle_failure(
    &self,
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  );
}

#[derive(Debug, Clone)]
pub struct SupervisorStrategyHandle(Arc<dyn SupervisorStrategy>);

impl PartialEq for SupervisorStrategyHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SupervisorStrategyHandle {}

impl std::hash::Hash for SupervisorStrategyHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn SupervisorStrategy).hash(state);
  }
}

impl SupervisorStrategyHandle {
  pub fn new_arc(s: Arc<dyn SupervisorStrategy>) -> Self {
    SupervisorStrategyHandle(s)
  }

  pub fn new(s: impl SupervisorStrategy + 'static) -> Self {
    SupervisorStrategyHandle(Arc::new(s))
  }
}

#[async_trait]
impl SupervisorStrategy for SupervisorStrategyHandle {
  async fn handle_failure(
    &self,
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  ) {
    self
      .0
      .handle_failure(actor_system, supervisor, child, rs, reason, message)
      .await;
  }
}

#[async_trait]
pub trait Supervisor: Send + Sync + 'static {
  async fn get_children(&self) -> Vec<ExtendedPid>;
  async fn escalate_failure(&mut self, reason: ActorInnerError, message: MessageHandle);
  async fn restart_children(&self, pids: &[ExtendedPid]);
  async fn stop_children(&self, pids: &[ExtendedPid]);
  async fn resume_children(&self, pids: &[ExtendedPid]);
}

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

  async fn escalate_failure(&mut self, reason: ActorInnerError, message: MessageHandle) {
    let mut mg = self.0.lock().await;
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

pub fn default_decider(_: ActorInnerError) -> Directive {
  Directive::Restart
}

pub static DEFAULT_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> = Lazy::new(|| {
  SupervisorStrategyHandle::new(OneForOneStrategy::new(
    10,
    tokio::time::Duration::from_secs(10),
    DeciderFunc::new(default_decider),
  ))
});

pub static RESTARTING_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> =
  Lazy::new(|| SupervisorStrategyHandle::new(RestartingStrategy::new()));
