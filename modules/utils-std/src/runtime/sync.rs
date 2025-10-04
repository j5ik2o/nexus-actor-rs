use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::time::Duration;
use std::boxed::Box;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use super::spawn::TokioCoreSpawner;
use nexus_actor_core_rs::actor::core_types::restart::FailureClock;
use nexus_actor_core_rs::runtime::{
  AsyncMutex, AsyncNotify, AsyncRwLock, AsyncYield, CoreJoinHandle, CoreRuntime, CoreRuntimeConfig,
  CoreScheduledHandle, CoreScheduledHandleRef, CoreScheduledTask, CoreScheduler, CoreSpawner, CoreTaskFuture, Timer,
};
use tokio::sync::{Mutex as TokioMutexRaw, MutexGuard, Notify, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub struct TokioMutex<T>(TokioMutexRaw<T>);

impl<T> TokioMutex<T> {
  pub fn new(value: T) -> Self {
    Self(TokioMutexRaw::new(value))
  }

  pub fn into_inner(self) -> TokioMutexRaw<T> {
    self.0
  }
}

impl<T: Send + 'static> AsyncMutex<T> for TokioMutex<T> {
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>> {
    Box::pin(self.0.lock())
  }
}

#[derive(Debug)]
pub struct TokioRwLock<T>(RwLock<T>);

impl<T> TokioRwLock<T> {
  pub fn new(value: T) -> Self {
    Self(RwLock::new(value))
  }
}

impl<T: Send + Sync + 'static> AsyncRwLock<T> for TokioRwLock<T> {
  type ReadGuard<'a>
    = RwLockReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>> {
    Box::pin(self.0.read())
  }

  fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>> {
    Box::pin(self.0.write())
  }
}

#[derive(Debug)]
pub struct TokioNotify(Notify);

impl TokioNotify {
  pub fn new() -> Self {
    Self(Notify::new())
  }

  pub fn into_inner(self) -> Notify {
    self.0
  }
}

impl AsyncNotify for TokioNotify {
  fn notify_one(&self) {
    self.0.notify_one();
  }

  fn notify_waiters(&self) {
    self.0.notify_waiters();
  }

  fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(self.0.notified())
  }
}

#[derive(Debug, Default)]
pub struct TokioTimer;

impl Timer for TokioTimer {
  fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(tokio::time::sleep(duration))
  }
}

#[derive(Debug, Clone)]
pub struct InstantFailureClock {
  anchor: Arc<Mutex<Instant>>,
}

impl InstantFailureClock {
  pub fn new() -> Self {
    Self::with_anchor(Instant::now())
  }

  pub fn with_anchor(anchor: Instant) -> Self {
    Self {
      anchor: Arc::new(Mutex::new(anchor)),
    }
  }

  pub fn duration_since_anchor(&self, instant: Instant) -> Duration {
    let mut guard = self.anchor.lock().expect("anchor mutex poisoned");
    if instant < *guard {
      *guard = instant;
      Duration::from_secs(0)
    } else {
      instant.duration_since(*guard)
    }
  }
}

impl FailureClock for InstantFailureClock {
  fn now(&self) -> Duration {
    let guard = self.anchor.lock().expect("anchor mutex poisoned");
    Instant::now()
      .checked_duration_since(*guard)
      .unwrap_or_else(|| Duration::from_secs(0))
  }

  fn as_any(&self) -> &dyn core::any::Any {
    self
  }
}

#[derive(Default)]
pub struct TokioScheduler {
  state: Arc<TokioSchedulerState>,
}

impl fmt::Debug for TokioScheduler {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TokioScheduler").finish()
  }
}

struct TokioSchedulerState {
  handles: Mutex<Vec<Weak<dyn CoreJoinHandle>>>,
}

impl TokioSchedulerState {
  fn new() -> Self {
    Self {
      handles: Mutex::new(Vec::new()),
    }
  }

  fn register(&self, handle: &Arc<dyn CoreJoinHandle>) {
    let mut guard = self.handles.lock().expect("handles mutex poisoned");
    guard.push(Arc::downgrade(handle));
    guard.retain(|weak| weak.strong_count() > 0);
  }

  fn cancel_all(&self) {
    let mut guard = self.handles.lock().expect("handles mutex poisoned");
    for weak in guard.iter() {
      if let Some(handle) = weak.upgrade() {
        handle.cancel();
      }
    }
    guard.retain(|weak| weak.strong_count() > 0);
  }

  fn prune(&self) {
    let mut guard = self.handles.lock().expect("handles mutex poisoned");
    guard.retain(|weak| weak.strong_count() > 0);
  }
}

impl Default for TokioSchedulerState {
  fn default() -> Self {
    Self::new()
  }
}

struct TokioScheduledHandle {
  handle: Arc<dyn CoreJoinHandle>,
  state: Arc<TokioSchedulerState>,
}

impl TokioScheduledHandle {
  fn new(handle: Arc<dyn CoreJoinHandle>, state: Arc<TokioSchedulerState>) -> Self {
    state.register(&handle);
    Self { handle, state }
  }
}

impl CoreScheduledHandle for TokioScheduledHandle {
  fn cancel(&self) {
    self.handle.cancel();
    self.state.prune();
  }

  fn is_cancelled(&self) -> bool {
    self.handle.is_finished()
  }

  fn is_active(&self) -> bool {
    !self.handle.is_finished()
  }
}

impl Drop for TokioScheduledHandle {
  fn drop(&mut self) {
    self.state.prune();
  }
}

impl TokioScheduler {
  pub fn new() -> Self {
    Self {
      state: Arc::new(TokioSchedulerState::default()),
    }
  }

  fn spawn_task<F>(&self, future: F) -> Arc<dyn CoreJoinHandle>
  where
    F: Future<Output = ()> + Send + 'static, {
    let task: CoreTaskFuture = Box::pin(future);
    TokioCoreSpawner::current()
      .spawn(task)
      .expect("Tokio scheduler failed to spawn task")
  }

  fn wrap_handle(&self, handle: Arc<dyn CoreJoinHandle>) -> CoreScheduledHandleRef {
    Arc::new(TokioScheduledHandle::new(handle, self.state.clone())) as CoreScheduledHandleRef
  }

  fn run_task(task: CoreScheduledTask) -> CoreTaskFuture {
    (task)()
  }
}

impl CoreScheduler for TokioScheduler {
  fn schedule_once(&self, delay: Duration, task: CoreScheduledTask) -> CoreScheduledHandleRef {
    let future = async move {
      if !delay.is_zero() {
        tokio::time::sleep(delay).await;
      }
      TokioScheduler::run_task(task).await;
    };
    self.wrap_handle(self.spawn_task(future))
  }

  fn schedule_repeated(
    &self,
    initial_delay: Duration,
    interval: Duration,
    task: CoreScheduledTask,
  ) -> CoreScheduledHandleRef {
    let future = async move {
      if !initial_delay.is_zero() {
        tokio::time::sleep(initial_delay).await;
      }
      loop {
        TokioScheduler::run_task(task.clone()).await;
        if interval.is_zero() {
          break;
        }
        tokio::time::sleep(interval).await;
      }
    };
    self.wrap_handle(self.spawn_task(future))
  }

  fn drain(&self) {
    self.state.cancel_all();
  }
}

#[derive(Debug, Default)]
pub struct TokioYield;

impl AsyncYield for TokioYield {
  fn yield_now(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(tokio::task::yield_now())
  }
}

#[derive(Debug, Clone)]
pub struct TokioRuntime {
  scheduler: Arc<TokioScheduler>,
  timer: Arc<TokioTimer>,
  yielder: Arc<TokioYield>,
  failure_clock: Arc<InstantFailureClock>,
}

impl TokioRuntime {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn scheduler(&self) -> Arc<TokioScheduler> {
    self.scheduler.clone()
  }

  pub fn timer(&self) -> Arc<TokioTimer> {
    self.timer.clone()
  }

  pub fn core_runtime(&self) -> CoreRuntime {
    let timer: Arc<dyn Timer> = self.timer.clone();
    let scheduler: Arc<dyn CoreScheduler> = self.scheduler.clone();
    let failure_clock: Arc<dyn FailureClock> = self.failure_clock.clone();
    let spawner: Arc<dyn CoreSpawner> = Arc::new(TokioCoreSpawner::current());
    let config = CoreRuntimeConfig::new(timer, scheduler)
      .with_yielder(self.yielder.clone() as Arc<dyn AsyncYield>)
      .with_spawner(spawner)
      .with_failure_clock(failure_clock);
    CoreRuntime::from(config)
  }
}

impl Default for TokioRuntime {
  fn default() -> Self {
    Self {
      scheduler: Arc::new(TokioScheduler::default()),
      timer: Arc::new(TokioTimer::default()),
      yielder: Arc::new(TokioYield::default()),
      failure_clock: Arc::new(InstantFailureClock::new()),
    }
  }
}

pub fn tokio_core_runtime() -> CoreRuntime {
  TokioRuntime::default().core_runtime()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicUsize, Ordering};

  fn make_task(counter: Arc<AtomicUsize>) -> CoreScheduledTask {
    Arc::new(move || {
      let counter = counter.clone();
      Box::pin(async move {
        counter.fetch_add(1, Ordering::SeqCst);
      })
    })
  }

  #[tokio::test]
  async fn schedule_once_executes_and_completes() {
    let scheduler = TokioScheduler::default();
    let counter = Arc::new(AtomicUsize::new(0));
    let handle = scheduler.schedule_once(Duration::from_millis(10), make_task(counter.clone()));

    tokio::time::sleep(Duration::from_millis(40)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert!(!handle.is_active());
    assert!(handle.is_cancelled());
  }

  #[tokio::test]
  async fn drain_cancels_pending_tasks() {
    let scheduler = TokioScheduler::default();
    let counter = Arc::new(AtomicUsize::new(0));
    let handle = scheduler.schedule_repeated(Duration::from_secs(1), Duration::from_secs(1), make_task(counter));

    scheduler.drain();
    tokio::time::sleep(Duration::from_millis(20)).await;

    assert!(!handle.is_active());
    assert!(handle.is_cancelled());
  }
}
