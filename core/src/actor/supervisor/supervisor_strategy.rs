use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use std::fmt::Debug;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::metrics::metrics_impl::{MetricsSink, SyncMetricsAccess};
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
    Fut: Future<Output = Directive> + Send + 'static,
  {
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

  fn metrics_access(&self) -> Option<&dyn SyncMetricsAccess> {
    None
  }
}

#[derive(Debug)]
struct SupervisorSnapshot {
  supervisor: Arc<dyn Supervisor>,
}

impl SupervisorSnapshot {
  fn new(supervisor: Arc<dyn Supervisor>) -> Self {
    Self { supervisor }
  }

  fn supervisor(&self) -> Arc<dyn Supervisor> {
    self.supervisor.clone()
  }
}

#[derive(Debug)]
pub struct SupervisorCell {
  supervisor: Arc<ArcSwapOption<SupervisorSnapshot>>,
  snapshot_hits: AtomicU64,
  snapshot_misses: AtomicU64,
}

impl SupervisorCell {
  pub fn replace_supervisor(&self, supervisor: Arc<dyn Supervisor>) {
    let snapshot = SupervisorSnapshot::new(supervisor);
    self.supervisor.store(Some(Arc::new(snapshot)));
  }

  pub fn load_supervisor(&self) -> Option<Arc<dyn Supervisor>> {
    match self.supervisor.load_full() {
      Some(snapshot) => {
        self.snapshot_hits.fetch_add(1, Ordering::Relaxed);
        Some(snapshot.supervisor())
      }
      None => {
        self.snapshot_misses.fetch_add(1, Ordering::Relaxed);
        None
      }
    }
  }

  pub fn snapshot_stats(&self) -> SupervisorCellStats {
    SupervisorCellStats {
      hits: self.snapshot_hits.load(Ordering::Relaxed),
      misses: self.snapshot_misses.load(Ordering::Relaxed),
    }
  }
}

impl Default for SupervisorCell {
  fn default() -> Self {
    Self {
      supervisor: Arc::new(ArcSwapOption::from(None::<Arc<SupervisorSnapshot>>)),
      snapshot_hits: AtomicU64::new(0),
      snapshot_misses: AtomicU64::new(0),
    }
  }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SupervisorCellStats {
  pub hits: u64,
  pub misses: u64,
}

#[derive(Debug, Clone)]
pub struct SupervisorHandle {
  inner: Arc<RwLock<dyn Supervisor>>,
  cell: Arc<SupervisorCell>,
}

impl SupervisorHandle {
  pub async fn get_supervisor(&self) -> Arc<RwLock<dyn Supervisor>> {
    self.inner.clone()
  }

  pub async fn borrow(&self) -> SupervisorBorrow<'_> {
    SupervisorBorrow {
      guard: self.inner.read().await,
    }
  }

  pub async fn borrow_mut(&self) -> SupervisorBorrowMut<'_> {
    SupervisorBorrowMut {
      guard: self.inner.write().await,
    }
  }

  pub fn supervisor_arc(&self) -> Option<Arc<dyn Supervisor>> {
    self.cell.load_supervisor()
  }

  pub fn supervisor_cell(&self) -> Arc<SupervisorCell> {
    self.cell.clone()
  }

  pub fn supervisor_cell_stats(&self) -> SupervisorCellStats {
    self.cell.snapshot_stats()
  }

  pub fn inject_snapshot(&self, supervisor: Arc<dyn Supervisor>) {
    self.cell.replace_supervisor(supervisor);
  }

  pub fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    SyncMetricsAccess::metrics_sink(self)
  }
}

impl SupervisorHandle {
  pub fn new_arc(s: Arc<RwLock<dyn Supervisor>>) -> Self {
    SupervisorHandle {
      inner: s,
      cell: Arc::new(SupervisorCell::default()),
    }
  }

  pub fn new<S>(s: S) -> Self
  where
    S: Supervisor + Clone + 'static,
  {
    SupervisorHandle {
      inner: Arc::new(RwLock::new(s)),
      cell: Arc::new(SupervisorCell::default()),
    }
  }
}

impl PartialEq for SupervisorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Eq for SupervisorHandle {}

impl std::hash::Hash for SupervisorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.inner.as_ref() as *const RwLock<dyn Supervisor>).hash(state);
  }
}

impl SyncMetricsAccess for SupervisorHandle {
  fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self
      .supervisor_arc()
      .and_then(|supervisor| supervisor.metrics_access().and_then(|access| access.metrics_sink()))
  }
}

pub struct SupervisorBorrow<'a> {
  guard: RwLockReadGuard<'a, dyn Supervisor>,
}

impl<'a> Deref for SupervisorBorrow<'a> {
  type Target = dyn Supervisor;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

pub struct SupervisorBorrowMut<'a> {
  guard: RwLockWriteGuard<'a, dyn Supervisor>,
}

impl<'a> Deref for SupervisorBorrowMut<'a> {
  type Target = dyn Supervisor;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

impl<'a> DerefMut for SupervisorBorrowMut<'a> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut *self.guard
  }
}

#[async_trait]
impl Supervisor for SupervisorHandle {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    let mg = self.inner.read().await;
    mg.get_children().await
  }

  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle) {
    let mg = self.inner.read().await;
    mg.escalate_failure(reason, message_handle).await;
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    let mg = self.inner.read().await;
    mg.restart_children(pids).await;
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    let mg = self.inner.read().await;
    mg.stop_children(pids).await;
  }

  async fn resume_children(&self, pids: &[ExtendedPid]) {
    let mg = self.inner.read().await;
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
