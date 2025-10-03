use alloc::boxed::Box;
use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll};

use embassy_executor::raw::TaskStorage;
use embassy_executor::{SendSpawner, SpawnError};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use nexus_actor_core_rs::runtime::{CoreJoinFuture, CoreJoinHandle, CoreSpawnError, CoreSpawner, CoreTaskFuture};

/// Generic CoreSpawner implementation backed by user-provided closures.
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

/// CoreJoinHandle implementation backed by user-provided callbacks.
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
    (self.cancel)();
  }

  fn is_finished(&self) -> bool {
    (self.is_finished)()
  }

  fn detach(self: Arc<Self>) {
    (self.detach)();
  }

  fn join(self: Arc<Self>) -> CoreJoinFuture {
    (self.join)()
  }
}

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
