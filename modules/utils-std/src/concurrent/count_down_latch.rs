use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nexus_utils_core_rs::{async_trait, CountDownLatch as CoreCountDownLatch, CountDownLatchBackend};
use tokio::sync::Notify;

/// Backend implementation of countdown latch using Tokio runtime
///
/// A synchronization primitive that causes tasks to wait until the specified number of countdowns complete.
/// When the count reaches zero, all waiting tasks are released.
#[derive(Clone)]
pub struct TokioCountDownLatchBackend {
  inner: Arc<State>,
}

struct State {
  count: AtomicUsize,
  notify: Notify,
}

#[async_trait(?Send)]
impl CountDownLatchBackend for TokioCountDownLatchBackend {
  fn new(count: usize) -> Self {
    Self {
      inner: Arc::new(State {
        count: AtomicUsize::new(count),
        notify: Notify::new(),
      }),
    }
  }

  async fn count_down(&self) {
    let state = self.inner.clone();
    let prev = state.count.fetch_sub(1, Ordering::SeqCst);
    assert!(
      prev > 0,
      "CountDownLatch::count_down called more times than initial count"
    );
    if prev == 1 {
      state.notify.notify_waiters();
    }
  }

  async fn wait(&self) {
    let state = self.inner.clone();
    loop {
      if state.count.load(Ordering::SeqCst) == 0 {
        break;
      }
      let notified = state.notify.notified();
      if state.count.load(Ordering::SeqCst) == 0 {
        break;
      }
      notified.await;
    }
  }
}

/// Countdown latch using Tokio runtime
///
/// A synchronization primitive that causes tasks to wait until the specified number of countdowns complete.
/// When `count_down()` is called as many times as the initial count, all tasks waiting on `wait()` are released.
pub type CountDownLatch = CoreCountDownLatch<TokioCountDownLatchBackend>;

#[cfg(test)]
mod tests {
  use super::CountDownLatch;
  use tokio::join;

  #[tokio::test]
  async fn latch_reaches_zero() {
    let latch = CountDownLatch::new(2);
    let latch_clone = latch.clone();
    let wait_fut = latch.wait();
    let worker = async move {
      latch_clone.count_down().await;
      latch_clone.count_down().await;
    };

    join!(worker, wait_fut);
  }
}
