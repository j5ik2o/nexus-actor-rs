#![cfg(feature = "arc")]

use alloc::boxed::Box;
use alloc::sync::Arc;

use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{AsyncBarrier as CoreAsyncBarrier, AsyncBarrierBackend, BoxFuture};

pub struct ArcAsyncBarrierBackend<RM>
where
  RM: RawMutex,
{
  remaining: Arc<AtomicUsize>,
  initial: usize,
  signal: Arc<Signal<RM, ()>>,
}

impl<RM> Clone for ArcAsyncBarrierBackend<RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      remaining: self.remaining.clone(),
      initial: self.initial,
      signal: self.signal.clone(),
    }
  }
}

impl<RM> AsyncBarrierBackend for ArcAsyncBarrierBackend<RM>
where
  RM: RawMutex + Send + Sync,
{
  type WaitFuture<'a>
    = BoxFuture<'a, ()>
  where
    Self: 'a;

  fn new(count: usize) -> Self {
    assert!(count > 0, "AsyncBarrier must have positive count");
    Self {
      remaining: Arc::new(AtomicUsize::new(count)),
      initial: count,
      signal: Arc::new(Signal::new()),
    }
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    let remaining = self.remaining.clone();
    let signal = self.signal.clone();
    let initial = self.initial;
    Box::pin(async move {
      let prev = remaining.fetch_sub(1, Ordering::SeqCst);
      assert!(prev > 0, "AsyncBarrier::wait called more times than count");
      if prev == 1 {
        remaining.store(initial, Ordering::SeqCst);
        signal.signal(());
      } else {
        loop {
          if remaining.load(Ordering::SeqCst) == initial {
            break;
          }
          signal.wait().await;
        }
      }
    })
  }
}

pub type ArcLocalAsyncBarrier = CoreAsyncBarrier<ArcAsyncBarrierBackend<CriticalSectionRawMutex>>;
pub type ArcCsAsyncBarrier = ArcLocalAsyncBarrier;

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::ArcLocalAsyncBarrier;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn barrier_releases_all() {
    block_on(async {
      let barrier = ArcLocalAsyncBarrier::new(2);
      let other = barrier.clone();

      let first = barrier.wait();
      let second = other.wait();

      join!(first, second);
    });
  }
}
