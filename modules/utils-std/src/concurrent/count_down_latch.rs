use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nexus_utils_core_rs::{BoxFuture, CountDownLatch as CoreCountDownLatch, CountDownLatchBackend};
use tokio::sync::Notify;

#[derive(Clone)]
pub struct TokioCountDownLatchBackend {
  inner: Arc<State>,
}

struct State {
  count: AtomicUsize,
  notify: Notify,
}

impl CountDownLatchBackend for TokioCountDownLatchBackend {
  type CountDownFuture<'a>
    = BoxFuture<'a, ()>
  where
    Self: 'a;
  type WaitFuture<'a>
    = BoxFuture<'a, ()>
  where
    Self: 'a;

  fn new(count: usize) -> Self {
    Self {
      inner: Arc::new(State {
        count: AtomicUsize::new(count),
        notify: Notify::new(),
      }),
    }
  }

  fn count_down(&self) -> Self::CountDownFuture<'_> {
    let state = self.inner.clone();
    Box::pin(async move {
      let prev = state.count.fetch_sub(1, Ordering::SeqCst);
      assert!(
        prev > 0,
        "CountDownLatch::count_down called more times than initial count"
      );
      if prev == 1 {
        state.notify.notify_waiters();
      }
    })
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    let state = self.inner.clone();
    Box::pin(async move {
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
    })
  }
}

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
