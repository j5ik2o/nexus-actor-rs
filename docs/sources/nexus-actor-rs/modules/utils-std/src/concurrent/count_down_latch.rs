use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Clone)]
pub struct CountDownLatch {
  count: Arc<AtomicUsize>,
  notify: Arc<Notify>,
}

impl Debug for CountDownLatch {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CountDownLatch").field("count", &self.count).finish()
  }
}

impl Eq for CountDownLatch {}

impl PartialEq for CountDownLatch {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.count, &other.count)
  }
}

impl Default for CountDownLatch {
  fn default() -> Self {
    Self::new(0)
  }
}

impl CountDownLatch {
  pub fn new(count: usize) -> Self {
    Self {
      count: Arc::new(AtomicUsize::new(count)),
      notify: Arc::new(Notify::new()),
    }
  }

  pub async fn count_down(&self) {
    let prev = self.count.fetch_sub(1, Ordering::SeqCst);
    assert!(
      prev > 0,
      "CountDownLatch::count_down called more times than initial count"
    );
    if prev == 1 {
      self.notify.notify_waiters();
    }
  }

  pub async fn wait(&self) {
    while self.count.load(Ordering::SeqCst) != 0 {
      let notified = self.notify.notified();
      if self.count.load(Ordering::SeqCst) == 0 {
        break;
      }
      notified.await;
    }
  }
}
