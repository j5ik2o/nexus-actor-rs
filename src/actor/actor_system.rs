use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::Pid;
use crate::actor::context::root_context::RootContext;
use crate::actor::dispatch::dead_letter_process::DeadLetterProcess;
use crate::actor::event_stream::event_stream_process::EventStreamProcess;
use crate::actor::guardian::GuardiansValue;
use crate::actor::message::message_or_envelope::EMPTY_MESSAGE_HEADER;
use crate::actor::process::process_registry::ProcessRegistry;
use crate::actor::process::ProcessHandle;
use crate::actor::supervisor::supervision_event::subscribe_supervision;
use crate::ctxext::extensions::ContextExtensions;
use crate::event_stream::EventStream;

#[derive(Debug, Clone)]
struct ActorSystemInner {
  process_registry: Option<ProcessRegistry>,
  root_context: Option<RootContext>,
  event_stream: Arc<EventStream>,
  guardians: Option<GuardiansValue>,
  dead_letter: Option<DeadLetterProcess>,
  extensions: ContextExtensions,
  config: Config,
  id: String,
}

impl ActorSystemInner {
  fn new(config: Config) -> Self {
    let id = Uuid::new_v4().to_string();
    ActorSystemInner {
      id,
      config,
      process_registry: None,
      root_context: None,
      guardians: None,
      event_stream: Arc::new(EventStream::new()),
      dead_letter: None,
      extensions: ContextExtensions::new(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ActorSystem {
  inner: Arc<Mutex<ActorSystemInner>>,
}

impl ActorSystem {
  pub async fn new() -> Self {
    Self::new_config_options(&[]).await
  }

  pub async fn new_config_options(options: &[ConfigOption]) -> Self {
    let config = Self::configure(options);
    Self::new_with_config(config).await
  }

  pub async fn new_with_config(config: Config) -> Self {
    let system = Self {
      inner: Arc::new(Mutex::new(ActorSystemInner::new(config))),
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

    // if let Some(metrics_provider) = &config.metrics_provider {
    //   system.extensions.register(Metrics::new(metrics_provider.clone()));
    // }

    let event_stream_process = ProcessHandle::new(EventStreamProcess::new(system.clone()));
    system
      .get_process_registry()
      .await
      .add_process(event_stream_process, "eventstream");

    system
  }

  pub async fn new_local_pid(&self, id: &str) -> ExtendedPid {
    let pr = self.get_process_registry().await;
    let pid = Pid {
      id: id.to_string(),
      address: pr.get_address(),
      request_id: 0,
    };
    ExtendedPid::new(pid, self.clone())
  }

  pub async fn get_address(&self) -> String {
    self.get_process_registry().await.get_address()
  }

  pub async fn get_config(&self) -> Config {
    let inner_mg = self.inner.lock().await;
    inner_mg.config.clone()
  }

  pub async fn get_root_context(&self) -> RootContext {
    let inner_mg = self.inner.lock().await;
    inner_mg.root_context.as_ref().unwrap().clone()
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

  fn configure(options: &[ConfigOption]) -> Config {
    let mut config = Config::default();
    for option in options {
      option.apply(&mut config);
    }
    config
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
}

#[derive(Debug, Clone)]
pub struct Config {
  // pub metrics_provider: Option<Arc<dyn MetricsProvider>>,
  pub dispatcher_throughput: usize,
  pub dead_letter_throttle_interval: tokio::time::Duration,
  pub dead_letter_throttle_count: usize,
  pub dead_letter_request_logging: bool,
  pub developer_supervision_logging: bool,
  // Other fields...
}

impl Default for Config {
  fn default() -> Self {
    Config {
      // metrics_provider: None,
      dispatcher_throughput: 300,
      dead_letter_throttle_interval: tokio::time::Duration::from_secs(1),
      dead_letter_throttle_count: 10,
      dead_letter_request_logging: false,
      developer_supervision_logging: false,
      // Set other default values...
    }
  }
}

pub enum ConfigOption {
  // SetMetricsProvider(Arc<dyn MetricsProvider>),
  SetDispatcherThroughput(usize),
  SetDeadLetterThrottleInterval(tokio::time::Duration),
  SetDeadLetterThrottleCount(usize),
  // Other options...
}

impl ConfigOption {
  fn apply(&self, config: &mut Config) {
    match self {
      // ConfigOption::SetMetricsProvider(provider) => {
      //   config.metrics_provider = Some(Arc::clone(provider));
      // },
      ConfigOption::SetDispatcherThroughput(throughput) => {
        config.dispatcher_throughput = *throughput;
      }
      ConfigOption::SetDeadLetterThrottleInterval(interval) => {
        config.dead_letter_throttle_interval = *interval;
      }
      ConfigOption::SetDeadLetterThrottleCount(count) => {
        config.dead_letter_throttle_count = *count;
      } // Handle other options...
    }
  }
}
