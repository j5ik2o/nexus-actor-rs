use alloc::boxed::Box;
use alloc::rc::Rc;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{async_trait, WaitGroup as CoreWaitGroup, WaitGroupBackend};

#[derive(Clone)]
pub struct RcWaitGroupBackend {
  count: Rc<RefCell<usize>>,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

#[async_trait(?Send)]
impl WaitGroupBackend for RcWaitGroupBackend {
  fn new() -> Self {
    Self::with_count(0)
  }

  fn with_count(count: usize) -> Self {
    Self {
      count: Rc::new(RefCell::new(count)),
      signal: Rc::new(Signal::new()),
    }
  }

  fn add(&self, n: usize) {
    *self.count.borrow_mut() += n;
  }

  fn done(&self) {
    let mut count = self.count.borrow_mut();
    assert!(*count > 0, "WaitGroup::done called more times than add");
    *count -= 1;
    if *count == 0 {
      self.signal.signal(());
    }
  }

  async fn wait(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    loop {
      if *count.borrow() == 0 {
        return;
      }
      signal.wait().await;
    }
  }
}

pub type WaitGroup = CoreWaitGroup<RcWaitGroupBackend>;

#[cfg(test)]
mod tests {
  use super::WaitGroup;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn wait_group_completes() {
    block_on(async {
      let wg = WaitGroup::new();
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
