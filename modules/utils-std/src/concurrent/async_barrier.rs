use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nexus_utils_core_rs::{async_trait, AsyncBarrier as CoreAsyncBarrier, AsyncBarrierBackend};
use tokio::sync::Notify;

/// Backend implementation of async barrier using Tokio runtime
///
/// A synchronization primitive for multiple tasks to wait for each other.
/// Blocks all tasks until the specified number of tasks call `wait()`.
#[derive(Clone)]
pub struct TokioAsyncBarrierBackend {
  inner: Arc<Inner>,
}

struct Inner {
  remaining: AtomicUsize,
  initial: usize,
  notify: Notify,
}

#[async_trait(?Send)]
impl AsyncBarrierBackend for TokioAsyncBarrierBackend {
  fn new(count: usize) -> Self {
    assert!(count > 0, "AsyncBarrier must have positive count");
    Self {
      inner: Arc::new(Inner {
        remaining: AtomicUsize::new(count),
        initial: count,
        notify: Notify::new(),
      }),
    }
  }

  async fn wait(&self) {
    let inner = self.inner.clone();
    let prev = inner.remaining.fetch_sub(1, Ordering::SeqCst);
    assert!(prev > 0, "AsyncBarrier::wait called more times than count");
    if prev == 1 {
      inner.remaining.store(inner.initial, Ordering::SeqCst);
      inner.notify.notify_waiters();
    } else {
      loop {
        if inner.remaining.load(Ordering::SeqCst) == inner.initial {
          break;
        }
        inner.notify.notified().await;
      }
    }
  }
}

/// Async barrier using Tokio runtime
///
/// A synchronization primitive that causes all tasks to wait until the specified number of tasks arrive.
/// When all tasks reach the barrier, it resets to a reusable state.
pub type AsyncBarrier = CoreAsyncBarrier<TokioAsyncBarrierBackend>;

#[cfg(test)]
mod tests {
  use super::AsyncBarrier;
  use tokio::join;

  #[tokio::test]
  async fn barrier_releases_all() {
    let barrier = AsyncBarrier::new(2);
    let b2 = barrier.clone();

    let first = async move {
      barrier.wait().await;
    };
    let second = async move {
      b2.wait().await;
    };

    join!(first, second);
  }
}
