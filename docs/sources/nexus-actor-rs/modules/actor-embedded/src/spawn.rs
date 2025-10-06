use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll};
use core::time::Duration;

use embassy_executor::raw::TaskStorage;
use embassy_executor::{SendSpawner, SpawnError};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration as EmbassyDuration, Timer as EmbassyTimer};
use nexus_actor_core_rs::runtime::{
  CoreJoinFuture, CoreJoinHandle, CoreScheduledHandle, CoreScheduledHandleRef, CoreScheduledTask, CoreScheduler,
  CoreSpawnError, CoreSpawner, CoreTaskFuture,
};

/// Join state shared between the spawned Embassy task and the join handle.
struct EmbassyJoinState {
  cancel_flag: AtomicBool,
  done_flag: AtomicBool,
  cancel_signal: Signal<CriticalSectionRawMutex, ()>,
  completion: Signal<CriticalSectionRawMutex, ()>,
  slot: &'static EmbassyTaskSlot,
}

impl EmbassyJoinState {
  fn new(slot: &'static EmbassyTaskSlot) -> Self {
    Self {
      cancel_flag: AtomicBool::new(false),
      done_flag: AtomicBool::new(false),
      cancel_signal: Signal::new(),
      completion: Signal::new(),
      slot,
    }
  }

  fn cancel(&self) {
    if !self.cancel_flag.swap(true, Ordering::Release) {
      self.cancel_signal.signal(());
    }
  }

  fn is_cancelled(&self) -> bool {
    self.cancel_flag.load(Ordering::Acquire)
  }

  fn finish(&self) {
    if !self.done_flag.swap(true, Ordering::AcqRel) {
      self.slot.release();
      self.completion.signal(());
    }
  }

  fn is_finished(&self) -> bool {
    self.done_flag.load(Ordering::Acquire)
  }

  fn join_future(self: Arc<Self>) -> CoreJoinFuture {
    Box::pin(async move {
      if !self.is_finished() {
        self.completion.wait().await;
      }
    })
  }
}

struct EmbassyJoinHandle {
  state: Arc<EmbassyJoinState>,
}

impl EmbassyJoinHandle {
  fn new(state: Arc<EmbassyJoinState>) -> Self {
    Self { state }
  }
}

impl CoreJoinHandle for EmbassyJoinHandle {
  fn cancel(&self) {
    self.state.cancel();
  }

  fn is_finished(&self) -> bool {
    self.state.is_finished()
  }

  fn detach(self: Arc<Self>) {}

  fn join(self: Arc<Self>) -> CoreJoinFuture {
    EmbassyJoinState::join_future(self.state.clone())
  }
}

struct EmbassyTaskFuture {
  future: Option<CoreTaskFuture>,
  state: Arc<EmbassyJoinState>,
}

impl EmbassyTaskFuture {
  fn new(future: CoreTaskFuture, state: Arc<EmbassyJoinState>) -> Self {
    Self {
      future: Some(future),
      state,
    }
  }
}

impl Future for EmbassyTaskFuture {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    // Register for cancellation notifications.
    let cancel_ready = if self.state.is_cancelled() {
      true
    } else {
      let mut cancel_wait = self.state.cancel_signal.wait();
      unsafe { Pin::new_unchecked(&mut cancel_wait) }.poll(cx).is_ready()
    };

    if cancel_ready {
      self.future.take();
      self.state.finish();
      return Poll::Ready(());
    }

    match self.future.as_mut() {
      Some(inner) => match inner.as_mut().poll(cx) {
        Poll::Ready(()) => {
          self.future.take();
          self.state.finish();
          Poll::Ready(())
        }
        Poll::Pending => Poll::Pending,
      },
      None => Poll::Ready(()),
    }
  }
}

/// Static task slot used to execute CoreTaskFuture instances on an Embassy executor.
pub struct EmbassyTaskSlot {
  storage: TaskStorage<EmbassyTaskFuture>,
  busy: AtomicBool,
}

impl EmbassyTaskSlot {
  pub const fn new() -> Self {
    Self {
      storage: TaskStorage::new(),
      busy: AtomicBool::new(false),
    }
  }

  fn try_acquire(&self) -> bool {
    self
      .busy
      .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
      .is_ok()
  }

  fn release(&self) {
    self.busy.store(false, Ordering::Release);
  }

  fn spawn_future(
    &'static self,
    spawner: &SendSpawner,
    future: CoreTaskFuture,
  ) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> {
    let state = Arc::new(EmbassyJoinState::new(self));
    let state_clone = state.clone();
    let token = self.storage.spawn(|| EmbassyTaskFuture::new(future, state_clone));
    match spawner.spawn(token) {
      Ok(()) => Ok(Arc::new(EmbassyJoinHandle::new(state)) as Arc<dyn CoreJoinHandle>),
      Err(SpawnError::Busy) => {
        self.release();
        Err(CoreSpawnError::CapacityExhausted)
      }
    }
  }
}

/// CoreSpawner backed by a fixed set of Embassy task slots.
pub struct EmbassyCoreSpawner<const N: usize> {
  spawner: SendSpawner,
  slots: &'static [EmbassyTaskSlot; N],
}

impl<const N: usize> EmbassyCoreSpawner<N> {
  pub const fn new(spawner: SendSpawner, slots: &'static [EmbassyTaskSlot; N]) -> Self {
    Self { spawner, slots }
  }
}

impl<const N: usize> CoreSpawner for EmbassyCoreSpawner<N> {
  fn spawn(&self, task: CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> {
    if let Some(slot) = self.slots.iter().find(|slot| slot.try_acquire()) {
      slot.spawn_future(&self.spawner, task)
    } else {
      Err(CoreSpawnError::CapacityExhausted)
    }
  }
}

#[derive(Clone)]
pub struct EmbassyScheduler {
  spawner: Arc<dyn CoreSpawner>,
  state: Arc<EmbassySchedulerState>,
}

impl EmbassyScheduler {
  pub fn new(spawner: Arc<dyn CoreSpawner>) -> Self {
    Self {
      spawner,
      state: Arc::new(EmbassySchedulerState::default()),
    }
  }

  fn spawn_task<F>(&self, future: F) -> Arc<dyn CoreJoinHandle>
  where
    F: Future<Output = ()> + Send + 'static, {
    let task: CoreTaskFuture = Box::pin(future);
    self.spawner.spawn(task).expect("EmbassyScheduler failed to spawn task")
  }

  fn wrap_handle(&self, handle: Arc<dyn CoreJoinHandle>) -> CoreScheduledHandleRef {
    Arc::new(EmbassyScheduledHandle::new(handle, self.state.clone())) as CoreScheduledHandleRef
  }

  fn run_task(task: CoreScheduledTask) -> CoreTaskFuture {
    (task)()
  }

  fn sleep(duration: Duration) -> impl Future<Output = ()> {
    EmbassyTimer::after(to_embassy_duration(duration))
  }
}

impl CoreScheduler for EmbassyScheduler {
  fn schedule_once(&self, delay: Duration, task: CoreScheduledTask) -> CoreScheduledHandleRef {
    let future = async move {
      if !delay.is_zero() {
        Self::sleep(delay).await;
      }
      EmbassyScheduler::run_task(task).await;
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
        EmbassyScheduler::sleep(initial_delay).await;
      }
      loop {
        EmbassyScheduler::run_task(task.clone()).await;
        if interval.is_zero() {
          break;
        }
        EmbassyScheduler::sleep(interval).await;
      }
    };
    self.wrap_handle(self.spawn_task(future))
  }

  fn drain(&self) {
    self.state.cancel_all();
  }
}

struct EmbassySchedulerState {
  handles: BlockingMutex<CriticalSectionRawMutex, Vec<Weak<dyn CoreJoinHandle>>>,
}

impl EmbassySchedulerState {
  fn register(&self, handle: &Arc<dyn CoreJoinHandle>) {
    // SAFETY: `lock_mut` is never called reentrantly and the closure does not suspend.
    unsafe {
      self.handles.lock_mut(|handles| {
        handles.push(Arc::downgrade(handle));
        handles.retain(|weak| weak.strong_count() > 0);
      });
    }
  }

  fn cancel_all(&self) {
    let handles_to_cancel: Vec<Arc<dyn CoreJoinHandle>> = unsafe {
      self.handles.lock_mut(|handles| {
        let mut strong = Vec::with_capacity(handles.len());
        handles.retain(|weak| {
          if let Some(handle) = weak.upgrade() {
            strong.push(handle);
          }
          // drop existing entry regardless, since the caller will cancel all
          false
        });
        strong
      })
    };

    for handle in handles_to_cancel {
      handle.cancel();
    }
  }

  fn prune(&self) {
    unsafe {
      self
        .handles
        .lock_mut(|handles| handles.retain(|weak| weak.strong_count() > 0));
    }
  }
}

impl Default for EmbassySchedulerState {
  fn default() -> Self {
    Self {
      handles: BlockingMutex::new(Vec::new()),
    }
  }
}

struct EmbassyScheduledHandle {
  handle: Arc<dyn CoreJoinHandle>,
  state: Arc<EmbassySchedulerState>,
}

impl EmbassyScheduledHandle {
  fn new(handle: Arc<dyn CoreJoinHandle>, state: Arc<EmbassySchedulerState>) -> Self {
    state.register(&handle);
    Self { handle, state }
  }
}

impl CoreScheduledHandle for EmbassyScheduledHandle {
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

impl Drop for EmbassyScheduledHandle {
  fn drop(&mut self) {
    self.state.prune();
  }
}

fn to_embassy_duration(duration: Duration) -> EmbassyDuration {
  let secs = EmbassyDuration::from_secs(duration.as_secs());
  let nanos = EmbassyDuration::from_nanos(duration.subsec_nanos() as u64);
  secs.checked_add(nanos).unwrap_or(EmbassyDuration::MAX)
}
