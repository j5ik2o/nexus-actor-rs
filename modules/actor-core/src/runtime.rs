#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::time::Duration;

use crate::actor::core_types::restart::FailureClock;
pub use nexus_utils_core_rs::async_primitives::{AsyncMutex, AsyncNotify, AsyncRwLock, AsyncYield, Timer};

/// Future 型の共通表現。スケジューラが実行する非同期タスクはこの型を返す必要があります。
pub type CoreTaskFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
pub type CoreJoinFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// スケジューラが起動するタスクファクトリ。
/// 戻り値の Future は必ず `Send + 'static` である必要があります。
pub type CoreScheduledTask = Arc<dyn Fn() -> CoreTaskFuture + Send + Sync + 'static>;

/// Spawn 処理で返却される JoinHandle の共通インターフェース。
pub trait CoreJoinHandle: Send + Sync {
  /// タスクをキャンセルします。複数回呼び出しても安全です。
  fn cancel(&self);

  /// タスクが完了していれば true。
  fn is_finished(&self) -> bool {
    false
  }

  /// ハンドルを破棄し、結果の取得を放棄します。
  fn detach(self: Arc<Self>);

  /// タスク完了を待機します。キャンセル済みの場合は即座に完了します。
  fn join(self: Arc<Self>) -> CoreJoinFuture;
}

/// Spawn 失敗時のエラー。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpawnError {
  ExecutorUnavailable,
  CapacityExhausted,
  Rejected,
}

/// Spawn を提供する最小抽象。
pub trait CoreSpawner: Send + Sync + 'static {
  fn spawn(&self, task: CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError>;
}

/// スケジューラが返すハンドルの共通インターフェース。
pub trait CoreScheduledHandle: Send + Sync {
  /// タスクをキャンセルします。複数回呼び出しても安全です。
  fn cancel(&self);

  /// 実装側がキャンセル状態を追跡している場合は `true` を返します。
  fn is_cancelled(&self) -> bool {
    false
  }

  /// ハンドルがまだ有効であれば `true`。
  fn is_active(&self) -> bool {
    !self.is_cancelled()
  }
}

/// スケジュールドタスクを表す参照型。
pub type CoreScheduledHandleRef = Arc<dyn CoreScheduledHandle + Send + Sync + 'static>;

/// コアランタイムが要求する最小限のスケジューラ挙動を定義します。
pub trait CoreScheduler: Send + Sync + 'static {
  /// `delay` 後に一度だけ `task` を実行します。
  fn schedule_once(&self, delay: Duration, task: CoreScheduledTask) -> CoreScheduledHandleRef;

  /// `initial_delay` 後に `task` を実行し、その後 `interval` ごとに繰り返します。
  /// `interval == Duration::ZERO` の場合は一度だけ実行して完了します。
  fn schedule_repeated(
    &self,
    initial_delay: Duration,
    interval: Duration,
    task: CoreScheduledTask,
  ) -> CoreScheduledHandleRef;

  /// スケジューラが管理するタスクを全てキャンセルします。
  fn drain(&self) {}
}

/// ランタイムが保持すべき構成要素。
#[derive(Clone)]
pub struct CoreRuntimeConfig {
  timer: Arc<dyn Timer>,
  scheduler: Arc<dyn CoreScheduler>,
  yielder: Option<Arc<dyn AsyncYield>>,
  spawner: Arc<dyn CoreSpawner>,
  failure_clock: Option<Arc<dyn FailureClock>>,
}

impl fmt::Debug for CoreRuntimeConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let timer_ptr = Arc::as_ptr(&self.timer) as *const ();
    let scheduler_ptr = Arc::as_ptr(&self.scheduler) as *const ();
    let yielder_ptr = self
      .yielder
      .as_ref()
      .map(|arc| Arc::as_ptr(arc) as *const ())
      .unwrap_or(core::ptr::null());
    f.debug_struct("CoreRuntimeConfig")
      .field("timer", &timer_ptr)
      .field("scheduler", &scheduler_ptr)
      .field("yielder", &yielder_ptr)
      .field(
        "failure_clock",
        &self
          .failure_clock
          .as_ref()
          .map(|arc| Arc::as_ptr(arc) as *const ())
          .unwrap_or(core::ptr::null()),
      )
      .finish()
  }
}

impl CoreRuntimeConfig {
  /// 新しい構成を生成します。
  pub fn new(timer: Arc<dyn Timer>, scheduler: Arc<dyn CoreScheduler>) -> Self {
    Self {
      timer,
      scheduler,
      yielder: None,
      spawner: Arc::new(NoopSpawner),
      failure_clock: None,
    }
  }

  pub fn with_yielder(mut self, yielder: Arc<dyn AsyncYield>) -> Self {
    self.yielder = Some(yielder);
    self
  }

  pub fn with_failure_clock(mut self, failure_clock: Arc<dyn FailureClock>) -> Self {
    self.failure_clock = Some(failure_clock);
    self
  }

  pub fn with_spawner(mut self, spawner: Arc<dyn CoreSpawner>) -> Self {
    self.spawner = spawner;
    self
  }

  /// タイマープリミティブへの参照を返します。
  pub fn timer(&self) -> Arc<dyn Timer> {
    self.timer.clone()
  }

  /// スケジューラへの参照を返します。
  pub fn scheduler(&self) -> Arc<dyn CoreScheduler> {
    self.scheduler.clone()
  }

  pub fn yielder(&self) -> Option<Arc<dyn AsyncYield>> {
    self.yielder.clone()
  }

  pub fn spawner(&self) -> Arc<dyn CoreSpawner> {
    self.spawner.clone()
  }

  pub fn failure_clock(&self) -> Option<Arc<dyn FailureClock>> {
    self.failure_clock.clone()
  }
}

#[derive(Debug)]
#[allow(dead_code)]
struct NoopJoinHandle;

impl CoreJoinHandle for NoopJoinHandle {
  fn cancel(&self) {}

  fn detach(self: Arc<Self>) {}

  fn join(self: Arc<Self>) -> CoreJoinFuture {
    Box::pin(async move {})
  }
}

#[derive(Debug)]
struct NoopSpawner;

impl CoreSpawner for NoopSpawner {
  fn spawn(&self, _task: CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> {
    Err(CoreSpawnError::ExecutorUnavailable)
  }
}

/// [`CoreSpawner`] 実装をクロージャで定義したい場合の補助構造体。
#[derive(Clone)]
pub struct FnCoreSpawner {
  spawn_fn: Arc<dyn Fn(CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> + Send + Sync>,
}

impl FnCoreSpawner {
  pub fn new<F>(spawn_fn: F) -> Self
  where
    F: Fn(CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> + Send + Sync + 'static, {
    Self {
      spawn_fn: Arc::new(spawn_fn),
    }
  }
}

impl CoreSpawner for FnCoreSpawner {
  fn spawn(&self, task: CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> {
    (self.spawn_fn)(task)
  }
}

/// [`CoreJoinHandle`] をクロージャでラップするヘルパー。
#[derive(Clone)]
pub struct FnJoinHandle {
  cancel: Arc<dyn Fn() + Send + Sync>,
  is_finished: Arc<dyn Fn() -> bool + Send + Sync>,
  detach: Arc<dyn Fn() + Send + Sync>,
  join: Arc<dyn Fn() -> CoreJoinFuture + Send + Sync>,
}

impl FnJoinHandle {
  pub fn new<C, F, D, J>(cancel: C, is_finished: F, detach: D, join: J) -> Self
  where
    C: Fn() + Send + Sync + 'static,
    F: Fn() -> bool + Send + Sync + 'static,
    D: Fn() + Send + Sync + 'static,
    J: Fn() -> CoreJoinFuture + Send + Sync + 'static, {
    Self {
      cancel: Arc::new(cancel),
      is_finished: Arc::new(is_finished),
      detach: Arc::new(detach),
      join: Arc::new(join),
    }
  }
}

impl CoreJoinHandle for FnJoinHandle {
  fn cancel(&self) {
    (self.cancel)()
  }

  fn is_finished(&self) -> bool {
    (self.is_finished)()
  }

  fn detach(self: Arc<Self>) {
    (self.detach)()
  }

  fn join(self: Arc<Self>) -> CoreJoinFuture {
    (self.join)()
  }
}

impl From<CoreRuntimeConfig> for CoreRuntime {
  fn from(value: CoreRuntimeConfig) -> Self {
    Self::from_config(value)
  }
}

/// コアランタイムの実体。依存側からはこの構造体を通じて
/// タイマーとスケジューラへアクセスします。
#[derive(Clone)]
pub struct CoreRuntime {
  timer: Arc<dyn Timer>,
  scheduler: Arc<dyn CoreScheduler>,
  yielder: Option<Arc<dyn AsyncYield>>,
  spawner: Arc<dyn CoreSpawner>,
  failure_clock: Option<Arc<dyn FailureClock>>,
}

impl fmt::Debug for CoreRuntime {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let timer_ptr = Arc::as_ptr(&self.timer) as *const ();
    let scheduler_ptr = Arc::as_ptr(&self.scheduler) as *const ();
    let yielder_ptr = self
      .yielder
      .as_ref()
      .map(|arc| Arc::as_ptr(arc) as *const ())
      .unwrap_or(core::ptr::null());
    let spawner_ptr = Arc::as_ptr(&self.spawner) as *const ();
    f.debug_struct("CoreRuntime")
      .field("timer", &timer_ptr)
      .field("scheduler", &scheduler_ptr)
      .field("yielder", &yielder_ptr)
      .field("spawner", &spawner_ptr)
      .field(
        "failure_clock",
        &self
          .failure_clock
          .as_ref()
          .map(|arc| Arc::as_ptr(arc) as *const ())
          .unwrap_or(core::ptr::null()),
      )
      .finish()
  }
}

impl CoreRuntime {
  /// 構成からランタイムを生成します。
  pub fn from_config(config: CoreRuntimeConfig) -> Self {
    Self {
      timer: config.timer(),
      scheduler: config.scheduler(),
      yielder: config.yielder(),
      spawner: config.spawner(),
      failure_clock: config.failure_clock(),
    }
  }

  /// タイマーへアクセスします。
  pub fn timer(&self) -> Arc<dyn Timer> {
    self.timer.clone()
  }

  /// スケジューラへアクセスします。
  pub fn scheduler(&self) -> Arc<dyn CoreScheduler> {
    self.scheduler.clone()
  }

  pub fn yielder(&self) -> Option<Arc<dyn AsyncYield>> {
    self.yielder.clone()
  }

  pub fn spawner(&self) -> Arc<dyn CoreSpawner> {
    self.spawner.clone()
  }

  pub fn failure_clock(&self) -> Option<Arc<dyn FailureClock>> {
    self.failure_clock.clone()
  }
}
