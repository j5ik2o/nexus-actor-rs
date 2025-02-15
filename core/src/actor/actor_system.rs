use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::actor::actor::ExtendedPid;
use crate::actor::context::{RootContext, TypedRootContext};
use crate::actor::dispatch::DeadLetterProcess;
use crate::actor::event_stream::EventStreamProcess;
use crate::actor::guardian::GuardiansValue;
use crate::actor::message::EMPTY_MESSAGE_HEADER;
use crate::actor::metrics::metrics_impl::Metrics;
use crate::actor::process::process_registry::ProcessRegistry;
use crate::actor::process::ProcessHandle;
use crate::actor::supervisor::subscribe_supervision;
use crate::actor::{Config, ConfigOption};
use crate::event_stream::EventStream;
use crate::extensions::Extensions;
use crate::generated::actor::Pid;

#[derive(Debug, Clone)]
struct ActorSystemInner {
  process_registry: Option<ProcessRegistry>,
  root_context: Option<RootContext>,
  event_stream: Arc<EventStream>,
  guardians: Option<GuardiansValue>,
  dead_letter: Option<DeadLetterProcess>,
  extensions: Extensions,
  config: Config,
  id: String,
}

impl ActorSystemInner {
  async fn new(config: Config) -> Self {
    let id = Uuid::new_v4().to_string();
    Self {
      id: id.clone(),
      config,
      process_registry: None,
      root_context: None,
      guardians: None,
      event_stream: Arc::new(EventStream::new()),
      dead_letter: None,
      extensions: Extensions::new(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ActorSystem {
  inner: Arc<Mutex<ActorSystemInner>>,
}

impl ActorSystem {
  pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
    Self::new_config_options([]).await
  }

  pub async fn new_config_options(options: impl IntoIterator<Item = ConfigOption>) -> Result<Self, Box<dyn std::error::Error>> {
    let options = options.into_iter().collect::<Vec<_>>();
    let config = Config::from(options);
    Self::new_with_config(config).await
  }

  pub async fn new_with_config(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
    let system = Self {
      inner: Arc::new(Mutex::new(ActorSystemInner::new(config.clone()).await)),
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
      system
        .get_extensions()
        .await
        .register(Arc::new(Mutex::new(Metrics::new(system.clone()).await?)))
        .await;
    }

    let event_stream_process = ProcessHandle::new(EventStreamProcess::new(system.clone()));
    system
      .get_process_registry()
      .await
      .add_process(event_stream_process, "eventstream")
      .await;

    Ok(system)
  }

  pub async fn new_local_pid(&self, id: &str) -> ExtendedPid {
    let pr = self.get_process_registry().await;
    let pid = Pid {
      id: id.to_string(),
      address: pr.get_address().await,
      request_id: 0,
    };
    ExtendedPid::new(pid)
  }

  pub async fn get_id(&self) -> String {
    let inner_mg = self.inner.lock().await;
    inner_mg.id.clone()
  }

  pub async fn get_address(&self) -> String {
    self.get_process_registry().await.get_address().await
  }

  pub async fn get_config(&self) -> Config {
    let inner_mg = self.inner.lock().await;
    inner_mg.config.clone()
  }

  pub async fn get_root_context(&self) -> RootContext {
    let inner_mg = self.inner.lock().await;
    inner_mg.root_context.as_ref().unwrap().clone()
  }

  pub async fn get_typed_root_context(&self) -> TypedRootContext {
    self.get_root_context().await.to_typed()
  }

  pub async fn get_dead_letter(&self) -> ProcessHandle {
    let inner_mg = self.inner.lock().await;
    let dead_letter = inner_mg.dead_letter.as_ref().unwrap().clone();
    ProcessHandle::new(dead_letter)
  }

  pub async fn get_process_registry(&self) -> ProcessRegistry {
    let inner_mg = self.inner.lock().await;
    inner_mg.process_registry.as_ref().unwrap().clone()
  }

  pub async fn get_event_stream(&self) -> Arc<EventStream> {
    let inner_mg = self.inner.lock().await;
    inner_mg.event_stream.clone()
  }

  pub async fn get_guardians(&self) -> GuardiansValue {
    let inner_mg = self.inner.lock().await;
    inner_mg.guardians.as_ref().unwrap().clone()
  }

  async fn set_root_context(&self, root: RootContext) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.root_context = Some(root);
  }

  async fn set_process_registry(&self, process_registry: ProcessRegistry) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.process_registry = Some(process_registry);
  }

  async fn set_guardians(&self, guardians: GuardiansValue) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.guardians = Some(guardians);
  }

  async fn set_dead_letter(&self, dead_letter: DeadLetterProcess) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.dead_letter = Some(dead_letter);
  }

  pub async fn get_extensions(&self) -> Extensions {
    let inner_mg = self.inner.lock().await;
    inner_mg.extensions.clone()
  }
}
