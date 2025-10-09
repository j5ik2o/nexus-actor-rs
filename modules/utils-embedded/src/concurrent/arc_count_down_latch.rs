#![cfg(feature = "arc")]

use alloc::boxed::Box;
use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{async_trait, CountDownLatch as CoreCountDownLatch, CountDownLatchBackend};

/// Backend implementation for countdown latch using `Arc`
///
/// Provides countdown synchronization using embassy-sync primitives with `Arc`
/// for thread-safe reference counting. Threads wait until the count reaches zero.
///
/// # Type Parameters
///
/// * `RM` - Raw mutex type from embassy-sync
pub struct ArcCountDownLatchBackend<RM>
where
  RM: RawMutex, {
  count: Arc<Mutex<RM, usize>>,
  signal: Arc<Signal<RM, ()>>,
}

impl<RM> Clone for ArcCountDownLatchBackend<RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      count: self.count.clone(),
      signal: self.signal.clone(),
    }
  }
}

#[async_trait(?Send)]
impl<RM> CountDownLatchBackend for ArcCountDownLatchBackend<RM>
where
  RM: RawMutex + Send + Sync,
{
  fn new(count: usize) -> Self {
    Self {
      count: Arc::new(Mutex::new(count)),
      signal: Arc::new(Signal::new()),
    }
  }

  async fn count_down(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    let mut guard = count.lock().await;
    assert!(*guard > 0, "CountDownLatch::count_down called too many times");
    *guard -= 1;
    if *guard == 0 {
      signal.signal(());
    }
  }

  async fn wait(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    loop {
      {
        let guard = count.lock().await;
        if *guard == 0 {
          return;
        }
      }
      signal.wait().await;
    }
  }
}

/// Type alias for `Arc`-based countdown latch using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe countdown synchronization for embedded contexts.
pub type ArcLocalCountDownLatch = CoreCountDownLatch<ArcCountDownLatchBackend<CriticalSectionRawMutex>>;

/// Alias for `ArcLocalCountDownLatch` for consistency
///
/// Uses critical section mutex backend.
pub type ArcCsCountDownLatch = ArcLocalCountDownLatch;

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::ArcLocalCountDownLatch;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn latch_waits_for_completion() {
    block_on(async {
      let latch = ArcLocalCountDownLatch::new(2);
      let worker_latch = latch.clone();

      let wait_fut = latch.wait();
      let worker = async move {
        worker_latch.count_down().await;
        worker_latch.count_down().await;
      };

      join!(worker, wait_fut);
    });
  }
}
