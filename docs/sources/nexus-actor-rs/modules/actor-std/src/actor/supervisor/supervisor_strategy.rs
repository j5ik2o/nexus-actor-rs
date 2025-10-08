use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use std::fmt::Debug;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::ActorContext;
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageHandle;
use crate::actor::metrics::metrics_impl::MetricsRuntime;
use crate::actor::supervisor::core_adapters::StdSupervisorAdapter;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
use crate::actor::supervisor::strategy_restarting::RestartingStrategy;
use crate::actor::supervisor::supervision_event::SupervisorEvent;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use opentelemetry::KeyValue;

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

pub(crate) fn supervisor_actor_type(supervisor: &SupervisorHandle) -> Option<Arc<str>> {
  supervisor.supervisor_arc().and_then(|sup| {
    sup
      .as_any()
      .downcast_ref::<ActorContext>()
      .and_then(|ctx| ctx.actor_type_arc())
  })
}

pub(crate) fn record_supervisor_metrics(
  supervisor: &SupervisorHandle,
  strategy: &'static str,
  decision: &str,
  child: &CorePid,
  mut extra_labels: Vec<KeyValue>,
) {
  if let Some(runtime) = supervisor.metrics_runtime() {
    let actor_type = supervisor_actor_type(supervisor).map(|arc| arc.to_string());
    let mut labels = vec![
      KeyValue::new("supervisor.strategy", strategy),
      KeyValue::new("supervisor.decision", decision.to_string()),
      KeyValue::new("supervisor.child_pid", child.to_string()),
    ];
    labels.append(&mut extra_labels);
    let sink = runtime.sink_for_actor(actor_type.as_deref());
    sink.increment_actor_failure_with_additional_labels(&labels);
  }
}

#[async_trait]
pub trait Supervisor: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
  async fn get_children(&self) -> Vec<CorePid>;
  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle);
  async fn restart_children(&self, pids: &[CorePid]);
  async fn stop_children(&self, pids: &[CorePid]);
  async fn resume_children(&self, pids: &[CorePid]);
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
  metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  snapshot_hits: AtomicU64,
  snapshot_misses: AtomicU64,
}

impl SupervisorCell {
  pub fn new(metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>) -> Self {
    Self {
      supervisor: Arc::new(ArcSwapOption::from(None::<Arc<SupervisorSnapshot>>)),
      metrics_runtime,
      snapshot_hits: AtomicU64::new(0),
      snapshot_misses: AtomicU64::new(0),
    }
  }

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

#[derive(Debug, Default, Clone, Copy)]
pub struct SupervisorCellStats {
  pub hits: u64,
  pub misses: u64,
}

#[derive(Debug, Clone)]
pub struct SupervisorHandle {
  cell: Arc<SupervisorCell>,
}

impl SupervisorHandle {
  pub fn get_supervisor(&self) -> Arc<dyn Supervisor> {
    self.supervisor_arc().expect("Supervisor snapshot is not initialized")
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

  pub fn replace_supervisor_arc(&self, supervisor: Arc<dyn Supervisor>) {
    self.cell.replace_supervisor(supervisor);
  }

  pub fn inject_snapshot(&self, supervisor: Arc<dyn Supervisor>) {
    self.replace_supervisor_arc(supervisor);
  }

  pub fn metrics_runtime(&self) -> Option<Arc<MetricsRuntime>> {
    self.cell.metrics_runtime.load_full()
  }

  pub fn core_adapter(&self) -> StdSupervisorAdapter {
    StdSupervisorAdapter::new(self.clone())
  }

  pub fn new_arc_with_metrics(
    supervisor: Arc<dyn Supervisor>,
    metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  ) -> Self {
    let cell = Arc::new(SupervisorCell::new(metrics_runtime));
    cell.replace_supervisor(supervisor);
    SupervisorHandle { cell }
  }

  pub fn new_arc(supervisor: Arc<dyn Supervisor>) -> Self {
    SupervisorHandle::new_arc_with_metrics(supervisor, Arc::new(ArcSwapOption::from(None::<Arc<MetricsRuntime>>)))
  }

  pub fn new_with_metrics<S>(supervisor: S, metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>) -> Self
  where
    S: Supervisor + Clone + 'static, {
    SupervisorHandle::new_arc_with_metrics(Arc::new(supervisor), metrics_runtime)
  }

  pub fn new<S>(supervisor: S) -> Self
  where
    S: Supervisor + Clone + 'static, {
    SupervisorHandle::new_arc(Arc::new(supervisor))
  }
}

impl PartialEq for SupervisorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.cell, &other.cell)
  }
}

impl Eq for SupervisorHandle {}

impl std::hash::Hash for SupervisorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.cell.as_ref() as *const SupervisorCell).hash(state);
  }
}

#[async_trait]
impl Supervisor for SupervisorHandle {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  async fn get_children(&self) -> Vec<CorePid> {
    self
      .supervisor_arc()
      .expect("Supervisor snapshot is not initialized")
      .get_children()
      .await
  }

  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle) {
    self
      .supervisor_arc()
      .expect("Supervisor snapshot is not initialized")
      .escalate_failure(reason, message_handle)
      .await;
  }

  async fn restart_children(&self, pids: &[CorePid]) {
    self
      .supervisor_arc()
      .expect("Supervisor snapshot is not initialized")
      .restart_children(pids)
      .await;
  }

  async fn stop_children(&self, pids: &[CorePid]) {
    self
      .supervisor_arc()
      .expect("Supervisor snapshot is not initialized")
      .stop_children(pids)
      .await;
  }

  async fn resume_children(&self, pids: &[CorePid]) {
    self
      .supervisor_arc()
      .expect("Supervisor snapshot is not initialized")
      .resume_children(pids)
      .await;
  }
}

pub async fn log_failure(actor_system: ActorSystem, child: &CorePid, reason: ErrorReason, directive: Directive) {
  let child_pid = ExtendedPid::from_core(child.clone());
  actor_system
    .get_event_stream()
    .await
    .publish(MessageHandle::new(SupervisorEvent {
      child: child_pid,
      reason,
      directive,
    }))
    .await;
}

pub static DEFAULT_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> =
  Lazy::new(|| SupervisorStrategyHandle::new(OneForOneStrategy::new(10, Duration::from_secs(10))));

pub static RESTARTING_SUPERVISION_STRATEGY: Lazy<SupervisorStrategyHandle> =
  Lazy::new(|| SupervisorStrategyHandle::new(RestartingStrategy::new()));

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::config::MetricsProvider;
  use crate::actor::ConfigOption;
  use opentelemetry_sdk::metrics::SdkMeterProvider;
  use tokio::runtime::Runtime;

  #[derive(Debug)]
  struct TestSupervisor;

  #[async_trait]
  impl Supervisor for TestSupervisor {
    fn as_any(&self) -> &dyn std::any::Any {
      self
    }

    async fn get_children(&self) -> Vec<CorePid> {
      Vec::new()
    }

    async fn escalate_failure(&self, _: ErrorReason, _: MessageHandle) {}

    async fn restart_children(&self, _: &[CorePid]) {}

    async fn stop_children(&self, _: &[CorePid]) {}

    async fn resume_children(&self, _: &[CorePid]) {}
  }

  fn make_runtime() -> Arc<MetricsRuntime> {
    let factory = Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
      let provider = Arc::new(MetricsProvider::Sdk(SdkMeterProvider::default()));
      let system = ActorSystem::new_config_options([ConfigOption::SetMetricsProvider(provider)])
        .await
        .expect("actor system");
      system.metrics_runtime().expect("metrics runtime")
    })
  }

  #[test]
  fn metrics_runtime_slot_upgrades_cleanly() {
    let slot = Arc::new(ArcSwapOption::from(None::<Arc<MetricsRuntime>>));
    let supervisor_arc: Arc<dyn Supervisor> = Arc::new(TestSupervisor);
    let handle = SupervisorHandle::new_arc_with_metrics(supervisor_arc.clone(), slot.clone());
    handle.inject_snapshot(supervisor_arc);

    assert!(handle.metrics_runtime().is_none());
    slot.store(Some(make_runtime()));
    assert!(handle.metrics_runtime().is_some());
  }

  #[test]
  fn loom_metrics_runtime_swap_is_race_free() {
    let runtime_arc = make_runtime();
    loom::model(move || {
      let slot = Arc::new(ArcSwapOption::from(Some(runtime_arc.clone())));
      let supervisor_arc: Arc<dyn Supervisor> = Arc::new(TestSupervisor);
      let handle = SupervisorHandle::new_arc_with_metrics(supervisor_arc.clone(), slot.clone());
      handle.inject_snapshot(supervisor_arc);

      let reader = {
        let handle = handle.clone();
        loom::thread::spawn(move || {
          let _ = handle.metrics_runtime();
        })
      };

      let writer = {
        let slot = slot.clone();
        loom::thread::spawn(move || {
          slot.store(None);
        })
      };

      reader.join().unwrap();
      writer.join().unwrap();
    });
  }
}
