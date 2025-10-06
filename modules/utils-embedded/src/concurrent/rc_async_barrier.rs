#![cfg(feature = "rc")]

use alloc::boxed::Box;
use alloc::rc::Rc;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::concurrent::{AsyncBarrier as CoreAsyncBarrier, AsyncBarrierBackend, BoxFuture};

#[derive(Clone)]
pub struct RcAsyncBarrierBackend {
  remaining: Rc<RefCell<usize>>,
  initial: usize,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

impl AsyncBarrierBackend for RcAsyncBarrierBackend {
  type WaitFuture<'a>
    = BoxFuture<'a, ()>
  where
    Self: 'a;

  fn new(count: usize) -> Self {
    assert!(count > 0, "AsyncBarrier must have positive count");
    Self {
      remaining: Rc::new(RefCell::new(count)),
      initial: count,
      signal: Rc::new(Signal::new()),
    }
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    let remaining = self.remaining.clone();
    let signal = self.signal.clone();
    let initial = self.initial;
    Box::pin(async move {
      {
        let mut rem = remaining.borrow_mut();
        assert!(*rem > 0, "AsyncBarrier::wait called more times than count");
        *rem -= 1;
        if *rem == 0 {
          *rem = initial;
          signal.signal(());
          return;
        }
      }

      loop {
        signal.wait().await;
        if *remaining.borrow() == initial {
          break;
        }
      }
    })
  }
}

pub type AsyncBarrier = CoreAsyncBarrier<RcAsyncBarrierBackend>;

#[cfg(test)]
mod tests {
  use super::AsyncBarrier;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn barrier_releases_all() {
    block_on(async {
      let barrier = AsyncBarrier::new(2);
      let other = barrier.clone();

      let first = barrier.wait();
      let second = other.wait();

      join!(first, second);
    });
  }
}
