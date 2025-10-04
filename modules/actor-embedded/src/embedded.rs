use alloc::sync::Arc;
use core::time::Duration;
use nexus_actor_core_rs::runtime::{
  CoreRuntime, CoreRuntimeConfig, CoreScheduledHandle, CoreScheduledHandleRef, CoreScheduledTask, CoreScheduler,
  CoreSpawner,
};
use nexus_utils_core_rs::async_primitives::Timer;
use nexus_utils_embedded_rs::async_primitives::EmbassyTimerWrapper;

use crate::spawn::EmbassyScheduler;

/// Embassy 向け Runtime 設定を構築するためのビルダ。
pub struct EmbeddedRuntimeBuilder {
  timer: Option<Arc<dyn Timer>>,
  scheduler: Option<Arc<dyn CoreScheduler>>,
  spawner: Option<Arc<dyn CoreSpawner>>,
}

impl EmbeddedRuntimeBuilder {
  pub const fn new() -> Self {
    Self {
      timer: None,
      scheduler: None,
      spawner: None,
    }
  }

  pub fn with_timer(mut self, timer: Arc<dyn Timer>) -> Self {
    self.timer = Some(timer);
    self
  }

  pub fn with_scheduler(mut self, scheduler: Arc<dyn CoreScheduler>) -> Self {
    self.scheduler = Some(scheduler);
    self
  }

  pub fn with_spawner(mut self, spawner: Arc<dyn CoreSpawner>) -> Self {
    self.spawner = Some(spawner);
    self
  }

  pub fn build(self) -> CoreRuntimeConfig {
    let timer: Arc<dyn Timer> = self.timer.unwrap_or_else(|| Arc::new(EmbassyTimerWrapper));
    let spawner_opt = self.spawner;
    let scheduler: Arc<dyn CoreScheduler> = match (self.scheduler, spawner_opt.as_ref()) {
      (Some(scheduler), _) => scheduler,
      (None, Some(spawner)) => Arc::new(EmbassyScheduler::new(spawner.clone())),
      (None, None) => Arc::new(UnsupportedScheduler),
    };

    let mut config = CoreRuntimeConfig::new(timer, scheduler);
    if let Some(spawner) = spawner_opt {
      config = config.with_spawner(spawner);
    }
    config
  }

  pub fn build_runtime(self) -> CoreRuntime {
    CoreRuntime::from_config(self.build())
  }
}

impl Default for EmbeddedRuntimeBuilder {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Debug)]
struct UnsupportedScheduledHandle;

impl CoreScheduledHandle for UnsupportedScheduledHandle {
  fn cancel(&self) {
    panic!("Embassy scheduler not yet implemented");
  }
}

#[derive(Debug)]
struct UnsupportedScheduler;

impl CoreScheduler for UnsupportedScheduler {
  fn schedule_once(&self, _delay: Duration, _task: CoreScheduledTask) -> CoreScheduledHandleRef {
    Arc::new(UnsupportedScheduledHandle)
  }

  fn schedule_repeated(
    &self,
    _initial_delay: Duration,
    _interval: Duration,
    _task: CoreScheduledTask,
  ) -> CoreScheduledHandleRef {
    Arc::new(UnsupportedScheduledHandle)
  }
}
