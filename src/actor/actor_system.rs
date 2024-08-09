use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::actor::actor::ExtendedPid;
use crate::actor::actor::Pid;
use crate::actor::context::RootContext;
use crate::actor::dispatch::DeadLetterProcess;
use crate::actor::event_stream::EventStreamProcess;
use crate::actor::guardian::GuardiansValue;
use crate::actor::message::EMPTY_MESSAGE_HEADER;
use crate::actor::process::process_registry::ProcessRegistry;
use crate::actor::process::ProcessHandle;
use crate::actor::supervisor::subscribe_supervision;
use crate::ctxext::extensions::ContextExtensions;
use crate::event_stream::event_stream::EventStream;
use crate::log::io_encoder::IoEncoder;
use crate::log::log::{LogLevel, Logger};
use crate::log::log_event::LogEvent;
use crate::log::log_event_stream::LogEventStream;

pub async fn write_event(event: &LogEvent) -> Vec<u8> {
  let mut buf = Vec::new();
  if !event.message.is_empty() {
    buf.extend_from_slice(event.message.as_bytes());
    buf.push(b' ');
  }
  let mut encoder = IoEncoder::new(&mut buf);
  for field in &event.context {
    field.encode(&mut encoder);
    encoder.write_space();
  }
  for field in &event.fields {
    field.encode(&mut encoder);
    encoder.write_space();
  }
  buf
}

#[derive(Debug, Clone)]
struct ActorSystemInner {
  process_registry: Option<ProcessRegistry>,
  root_context: Option<RootContext>,
  event_stream: Arc<EventStream>,
  log_event_stream: Arc<LogEventStream>,
  logger: Arc<Logger>,
  guardians: Option<GuardiansValue>,
  dead_letter: Option<DeadLetterProcess>,
  extensions: ContextExtensions,
  config: Config,
  id: String,
}

impl ActorSystemInner {
  async fn new(config: Config) -> Self {
    let id = Uuid::new_v4().to_string();

    let log_event_stream = LogEventStream::new();
    let logger = Arc::new(Logger::new(
      log_event_stream.clone(),
      config.log_level,
      &config.log_prefix,
    ));

    log_event_stream
      .subscribe(|event| async move {
        let buf = write_event(&event).await;
        let buf = std::str::from_utf8(&buf).unwrap();
        match event.level {
          LogLevel::Off => {}
          LogLevel::Debug => tracing::debug!("{}", buf),
          LogLevel::Info => tracing::info!("{}", buf),
          LogLevel::Warn => tracing::warn!("{}", buf),
          LogLevel::Error => tracing::error!("{}", buf),
          _ => {}
        }
      })
      .await;
    let myself = ActorSystemInner {
      id: id.clone(),
      config,
      process_registry: None,
      root_context: None,
      guardians: None,
      event_stream: Arc::new(EventStream::new()),
      logger: logger.clone(),
      log_event_stream,
      dead_letter: None,
      extensions: ContextExtensions::new(),
    };
    myself
  }
}

#[derive(Debug, Clone)]
pub struct ActorSystem {
  inner: Arc<Mutex<ActorSystemInner>>,
}

impl ActorSystem {
  pub async fn new() -> Self {
    Self::new_config_options([]).await
  }

  pub async fn new_config_options(options: impl IntoIterator<Item = ConfigOption>) -> Self {
    let options = options.into_iter().collect::<Vec<_>>();
    let config = Self::configure(options);
    Self::new_with_config(config).await
  }

  pub async fn new_with_config(config: Config) -> Self {
    let system = Self {
      inner: Arc::new(Mutex::new(ActorSystemInner::new(config).await)),
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

  pub async fn get_logger(&self) -> Arc<Logger> {
    self.inner.lock().await.logger.clone()
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

  fn configure(options: Vec<ConfigOption>) -> Config {
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
  pub log_level: LogLevel,
  pub log_prefix: String,
  pub dispatcher_throughput: usize,
  pub dead_letter_throttle_interval: Duration,
  pub dead_letter_throttle_count: usize,
  pub dead_letter_request_logging: bool,
  pub developer_supervision_logging: bool,
  // Other fields...
}

impl Default for Config {
  fn default() -> Self {
    Config {
      // metrics_provider: None,
      log_level: LogLevel::Info,
      log_prefix: "".to_string(),
      dispatcher_throughput: 300,
      dead_letter_throttle_interval: Duration::from_secs(1),
      dead_letter_throttle_count: 10,
      dead_letter_request_logging: false,
      developer_supervision_logging: false,
      // Set other default values...
    }
  }
}

pub enum ConfigOption {
  // SetMetricsProvider(Arc<dyn MetricsProvider>),
  SetLogLevel(LogLevel),
  SetLogPrefix(String),
  SetDispatcherThroughput(usize),
  SetDeadLetterThrottleInterval(Duration),
  SetDeadLetterThrottleCount(usize),
  // Other options...
}

impl ConfigOption {
  fn apply(&self, config: &mut Config) {
    match self {
      // ConfigOption::SetMetricsProvider(provider) => {
      //   config.metrics_provider = Some(Arc::clone(provider));
      // },
      ConfigOption::SetLogLevel(level) => {
        config.log_level = *level;
      }
      ConfigOption::SetLogPrefix(prefix) => {
        config.log_prefix = prefix.clone();
      }
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
