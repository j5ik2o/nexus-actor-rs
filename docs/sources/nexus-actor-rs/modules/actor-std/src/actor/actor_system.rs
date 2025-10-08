use arc_swap::{ArcSwap, ArcSwapOption};
use nexus_actor_core_rs::runtime::CoreRuntime;
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub(crate) mod registry;
pub use registry::with_actor_system;
use registry::{next_actor_system_id, register_actor_system};

pub type ActorSystemId = u64;

use crate::actor::context::{RootContext, TypedRootContext};
use crate::actor::core::ExtendedPid;
use crate::actor::dispatch::DeadLetterProcess;
use crate::actor::event_stream::EventStreamProcess;
use crate::actor::guardian::GuardiansValue;
use crate::actor::message::EMPTY_MESSAGE_HEADER;
use crate::actor::metrics::metrics_impl::{Metrics, MetricsRuntime};
use crate::actor::process::process_registry::ProcessRegistry;
use crate::actor::process::ProcessHandle;
use crate::actor::supervisor::subscribe_supervision;
use crate::actor::{Config, ConfigOption};
use crate::event_stream::EventStream;
use crate::extensions::Extensions;
use crate::generated::actor::Pid;
use crate::metrics::MetricsError;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
struct ActorSystemInner {
  process_registry: Option<ProcessRegistry>,
  root_context: Option<RootContext>,
  event_stream: Arc<EventStream>,
  dead_letter: Option<DeadLetterProcess>,
  extensions: Extensions,
  id: String,
  system_id: ActorSystemId,
  config: Arc<ArcSwap<Config>>,
  metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
}

impl ActorSystemInner {
  async fn new(
    config: Arc<ArcSwap<Config>>,
    metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
    system_id: ActorSystemId,
  ) -> Self {
    let id = Uuid::new_v4().to_string();
    Self {
      id: id.clone(),
      system_id,
      config,
      process_registry: None,
      root_context: None,
      event_stream: Arc::new(EventStream::new()),
      dead_letter: None,
      extensions: Extensions::new(),
      metrics_runtime,
    }
  }
}

#[derive(Debug, Clone)]
pub struct ActorSystem {
  inner: Arc<RwLock<ActorSystemInner>>,
  config: Arc<ArcSwap<Config>>,
  metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  guardians: Arc<ArcSwapOption<GuardiansValue>>,
  core_runtime: CoreRuntime,
  system_id: ActorSystemId,
}

#[derive(Debug, Clone)]
pub struct WeakActorSystem {
  inner: Weak<RwLock<ActorSystemInner>>,
  config: Weak<ArcSwap<Config>>,
  metrics_runtime: Weak<ArcSwapOption<MetricsRuntime>>,
  guardians: Weak<ArcSwapOption<GuardiansValue>>,
  core_runtime: CoreRuntime,
  system_id: ActorSystemId,
}

impl ActorSystem {
  pub async fn new() -> Result<Self, MetricsError> {
    Self::new_config_options([]).await
  }

  pub async fn new_config_options(options: impl IntoIterator<Item = ConfigOption>) -> Result<Self, MetricsError> {
    let options = options.into_iter().collect::<Vec<_>>();
    let config = Config::from(options);
    Self::new_with_config(config).await
  }

  pub async fn new_with_config(config: Config) -> Result<Self, MetricsError> {
    let config_swap = Arc::new(ArcSwap::from_pointee(config.clone()));
    let metrics_runtime_swap = Arc::new(ArcSwapOption::from(None::<Arc<MetricsRuntime>>));
    let guardians_swap = Arc::new(ArcSwapOption::from(None::<Arc<GuardiansValue>>));
    let core_runtime = config.core_runtime.clone();
    let system_id = next_actor_system_id();

    let system = Self {
      inner: Arc::new(RwLock::new(
        ActorSystemInner::new(config_swap.clone(), metrics_runtime_swap.clone(), system_id).await,
      )),
      config: config_swap.clone(),
      metrics_runtime: metrics_runtime_swap.clone(),
      guardians: guardians_swap.clone(),
      core_runtime,
      system_id,
    };
    system
      .set_root_context(RootContext::new(system.clone(), EMPTY_MESSAGE_HEADER.clone(), &[]))
      .await;
    system.set_process_registry(ProcessRegistry::new(system.clone())).await;
    system.set_guardians(GuardiansValue::new(system.clone())).await;
    system
      .set_dead_letter(DeadLetterProcess::new(system.clone()).await)
      .await;

    subscribe_supervision(&system).await;

    if config.metrics_provider.is_some() {
      let metrics = Metrics::new(system.clone(), metrics_runtime_swap.clone()).await?;
      system
        .get_extensions()
        .await
        .register(Arc::new(Mutex::new(metrics)))
        .await;
    }

    let event_stream_process = ProcessHandle::new(EventStreamProcess::new(system.clone()));
    system
      .get_process_registry()
      .await
      .add_process(event_stream_process, "eventstream")
      .await;

    register_actor_system(system_id, &system);

    Ok(system)
  }

  pub fn downgrade(&self) -> WeakActorSystem {
    WeakActorSystem {
      inner: Arc::downgrade(&self.inner),
      config: Arc::downgrade(&self.config),
      metrics_runtime: Arc::downgrade(&self.metrics_runtime),
      guardians: Arc::downgrade(&self.guardians),
      core_runtime: self.core_runtime.clone(),
      system_id: self.system_id,
    }
  }

  pub async fn new_local_pid(&self, id: &str) -> ExtendedPid {
    let pr = self.get_process_registry().await;
    let pid = Pid {
      id: id.to_string(),
      address: pr.get_address(),
      request_id: 0,
    };
    ExtendedPid::new(pid)
  }

  pub async fn get_id(&self) -> String {
    let inner_mg = self.inner.read().await;
    inner_mg.id.clone()
  }

  pub fn system_id(&self) -> ActorSystemId {
    self.system_id
  }

  pub async fn get_address(&self) -> String {
    self.get_process_registry().await.get_address()
  }

  pub fn get_config(&self) -> Config {
    self.config.load_full().as_ref().clone()
  }

  pub fn config_arc(&self) -> Arc<Config> {
    self.config.load_full()
  }

  pub fn metrics_runtime(&self) -> Option<Arc<MetricsRuntime>> {
    self.metrics_runtime.load_full()
  }

  pub(crate) fn metrics_runtime_slot(&self) -> Arc<ArcSwapOption<MetricsRuntime>> {
    self.metrics_runtime.clone()
  }

  pub(crate) fn guardians_slot(&self) -> Arc<ArcSwapOption<GuardiansValue>> {
    self.guardians.clone()
  }

  pub fn guardians_snapshot(&self) -> Option<GuardiansValue> {
    self.guardians.load_full().map(|guardians| (*guardians).clone())
  }

  pub fn metrics_foreach<R, F>(&self, f: F) -> Option<R>
  where
    F: FnOnce(&Arc<MetricsRuntime>) -> R, {
    let factory = self.metrics_runtime()?;
    Some(f(&runtime))
  }

  pub fn core_runtime(&self) -> CoreRuntime {
    self.core_runtime.clone()
  }

  pub async fn get_root_context(&self) -> RootContext {
    let inner_mg = self.inner.read().await;
    inner_mg.root_context.as_ref().unwrap().clone()
  }

  pub async fn get_typed_root_context(&self) -> TypedRootContext {
    self.get_root_context().await.to_typed()
  }

  pub async fn get_dead_letter(&self) -> ProcessHandle {
    let inner_mg = self.inner.read().await;
    let dead_letter = inner_mg.dead_letter.as_ref().unwrap().clone();
    ProcessHandle::new(dead_letter)
  }

  pub async fn get_process_registry(&self) -> ProcessRegistry {
    let inner_mg = self.inner.read().await;
    inner_mg.process_registry.as_ref().unwrap().clone()
  }

  pub async fn get_event_stream(&self) -> Arc<EventStream> {
    let inner_mg = self.inner.read().await;
    inner_mg.event_stream.clone()
  }

  pub async fn get_guardians(&self) -> GuardiansValue {
    self
      .guardians_snapshot()
      .expect("GuardiansValue must be initialized before access")
  }

  async fn set_root_context(&self, root: RootContext) {
    let mut inner_mg = self.inner.write().await;
    inner_mg.root_context = Some(root);
  }

  async fn set_process_registry(&self, process_registry: ProcessRegistry) {
    let mut inner_mg = self.inner.write().await;
    inner_mg.process_registry = Some(process_registry);
  }

  async fn set_guardians(&self, guardians: GuardiansValue) {
    self.guardians.store(Some(Arc::new(guardians)));
  }

  async fn set_dead_letter(&self, dead_letter: DeadLetterProcess) {
    let mut inner_mg = self.inner.write().await;
    inner_mg.dead_letter = Some(dead_letter);
  }

  pub async fn get_extensions(&self) -> Extensions {
    let inner_mg = self.inner.read().await;
    inner_mg.extensions.clone()
  }
}

impl WeakActorSystem {
  pub fn upgrade(&self) -> Option<ActorSystem> {
    let inner = self.inner.upgrade()?;
    let config = self.config.upgrade()?;
    let metrics_runtime = self.metrics_runtime.upgrade()?;
    let guardians = self.guardians.upgrade()?;
    Some(ActorSystem {
      inner,
      config,
      metrics_runtime,
      guardians,
      core_runtime: self.core_runtime.clone(),
      system_id: self.system_id,
    })
  }
}
