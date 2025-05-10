use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_condvar::Condvar;

#[derive(Clone)]
pub struct CountDownLatch {
  count: Arc<Mutex<usize>>,
  condvar: Arc<Condvar>,
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
      count: Arc::new(Mutex::new(count)),
      condvar: Arc::new(Condvar::new()),
    }
  }

  pub async fn count_down(&self) {
    let mut count = self.count.lock().await;
    *count -= 1;
    if *count == 0 {
      self.condvar.notify_all();
    }
  }

  pub async fn wait(&self) {
    let mut count = self.count.lock().await;
    while *count > 0 {
      count = self.condvar.wait(count).await;
    }
  }
}
