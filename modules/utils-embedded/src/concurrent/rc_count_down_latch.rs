use alloc::boxed::Box;
use alloc::rc::Rc;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::concurrent::{BoxFuture, CountDownLatch as CoreCountDownLatch, CountDownLatchBackend};

#[derive(Clone)]
pub struct RcCountDownLatchBackend {
  count: Rc<Mutex<NoopRawMutex, usize>>,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

impl CountDownLatchBackend for RcCountDownLatchBackend {
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
      count: Rc::new(Mutex::new(count)),
      signal: Rc::new(Signal::new()),
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

pub type CountDownLatch = CoreCountDownLatch<RcCountDownLatchBackend>;

#[cfg(test)]
mod tests {
  use super::CountDownLatch;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn latch_reaches_zero() {
    block_on(async {
      let latch = CountDownLatch::new(2);
      let clone = latch.clone();

      let wait_fut = latch.wait();
      let worker = async move {
        clone.count_down().await;
        clone.count_down().await;
      };

      join!(worker, wait_fut);
    });
  }
}
