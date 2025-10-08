#![cfg(feature = "arc")]

use alloc::boxed::Box;
use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{BoxFuture, CountDownLatch as CoreCountDownLatch, CountDownLatchBackend};

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

impl<RM> CountDownLatchBackend for ArcCountDownLatchBackend<RM>
where
  RM: RawMutex + Send + Sync,
{
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
      count: Arc::new(Mutex::new(count)),
      signal: Arc::new(Signal::new()),
    }
  }

  fn count_down(&self) -> Self::CountDownFuture<'_> {
    let count = self.count.clone();
    let signal = self.signal.clone();
    Box::pin(async move {
      let mut guard = count.lock().await;
      assert!(*guard > 0, "CountDownLatch::count_down called too many times");
      *guard -= 1;
      if *guard == 0 {
        signal.signal(());
      }
    })
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    let count = self.count.clone();
    let signal = self.signal.clone();
    Box::pin(async move {
      loop {
        {
          let guard = count.lock().await;
          if *guard == 0 {
            return;
          }
        }
        signal.wait().await;
      }
    })
  }
}

pub type ArcLocalCountDownLatch = CoreCountDownLatch<ArcCountDownLatchBackend<CriticalSectionRawMutex>>;
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
