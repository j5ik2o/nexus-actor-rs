#![cfg(feature = "arc")]

use alloc::boxed::Box;
use alloc::sync::Arc;

use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{BoxFuture, WaitGroup as CoreWaitGroup, WaitGroupBackend};

pub struct ArcWaitGroupBackend<RM>
where
  RM: RawMutex,
{
  count: Arc<AtomicUsize>,
  signal: Arc<Signal<RM, ()>>,
}

impl<RM> Clone for ArcWaitGroupBackend<RM>
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

impl<RM> WaitGroupBackend for ArcWaitGroupBackend<RM>
where
  RM: RawMutex + Send + Sync,
{
  type WaitFuture<'a>
    = BoxFuture<'a, ()>
  where
    Self: 'a;

  fn new() -> Self {
    Self::with_count(0)
  }

  fn with_count(count: usize) -> Self {
    Self {
      count: Arc::new(AtomicUsize::new(count)),
      signal: Arc::new(Signal::new()),
    }
  }

  fn add(&self, n: usize) {
    self.count.fetch_add(n, Ordering::SeqCst);
  }

  fn done(&self) {
    let prev = self.count.fetch_sub(1, Ordering::SeqCst);
    assert!(prev > 0, "WaitGroup::done called more times than add");
    if prev == 1 {
      self.signal.signal(());
    }
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    let count = self.count.clone();
    let signal = self.signal.clone();
    Box::pin(async move {
      loop {
        if count.load(Ordering::SeqCst) == 0 {
          return;
        }
        signal.wait().await;
      }
    })
  }
}

pub type ArcLocalWaitGroup = CoreWaitGroup<ArcWaitGroupBackend<CriticalSectionRawMutex>>;
pub type ArcCsWaitGroup = ArcLocalWaitGroup;

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::ArcLocalWaitGroup;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn wait_group_completes() {
    block_on(async {
      let wg = ArcLocalWaitGroup::new();
      wg.add(2);
      let clone = wg.clone();
      let worker = async move {
        clone.done();
        clone.done();
      };
      join!(worker, wg.wait());
    });
  }
}
