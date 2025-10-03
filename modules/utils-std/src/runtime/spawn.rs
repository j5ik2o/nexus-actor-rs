use std::sync::Arc;

use nexus_actor_core_rs::runtime::{CoreJoinFuture, CoreJoinHandle, CoreSpawnError, CoreSpawner, CoreTaskFuture};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Handle;
use tokio::task::JoinHandle as TokioJoinHandleImpl;

#[derive(Clone)]
pub struct TokioCoreSpawner {
  handle: Handle,
}

impl TokioCoreSpawner {
  pub fn new(handle: Handle) -> Self {
    Self { handle }
  }

  pub fn current() -> Self {
    Self::new(Handle::current())
  }
}

impl CoreSpawner for TokioCoreSpawner {
  fn spawn(&self, task: CoreTaskFuture) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError> {
    let join = self.handle.spawn(async move {
      task.await;
    });
    Ok(Arc::new(TokioJoinHandle::new(join)))
  }
}

struct TokioJoinHandle {
  inner: Mutex<Option<TokioJoinHandleImpl<()>>>,
  abort_on_drop: AtomicBool,
}

impl TokioJoinHandle {
  fn new(handle: TokioJoinHandleImpl<()>) -> Self {
    Self {
      inner: Mutex::new(Some(handle)),
      abort_on_drop: AtomicBool::new(false),
    }
  }

  fn with_handle<F, R>(&self, f: F) -> Option<R>
  where
    F: FnOnce(&TokioJoinHandleImpl<()>) -> R, {
    let guard = self.inner.lock();
    guard.as_ref().map(f)
  }
}

impl CoreJoinHandle for TokioJoinHandle {
  fn cancel(&self) {
    if let Some(handle) = self.inner.lock().take() {
      handle.abort();
    }
  }

  fn is_finished(&self) -> bool {
    self.with_handle(|h| h.is_finished()).unwrap_or(true)
  }

  fn detach(self: Arc<Self>) {
    self.abort_on_drop.store(false, Ordering::Release);
    let _ = self.inner.lock().take();
  }

  fn join(self: Arc<Self>) -> CoreJoinFuture {
    Box::pin(async move {
      let handle = {
        let mut guard = self.inner.lock();
        guard.take()
      };
      if let Some(handle) = handle {
        let _ = handle.await;
      }
    })
  }
}

impl Drop for TokioJoinHandle {
  fn drop(&mut self) {
    if self.abort_on_drop.load(Ordering::Acquire) {
      if let Some(handle) = self.inner.lock().take() {
        if !handle.is_finished() {
          handle.abort();
        }
      }
    }
  }
}
