use tokio::sync::Mutex;
use tokio_condvar::Condvar;

struct CountDownLatch {
  count: Mutex<usize>,
  condvar: Condvar,
}
impl CountDownLatch {
  fn new(count: usize) -> Self {
    Self {
      count: Mutex::new(count),
      condvar: Condvar::new(),
    }
  }

  async fn count_down(&self) {
    let mut count = self.count.lock().await;
    *count -= 1;
    if *count == 0 {
      self.condvar.notify_all();
    }
  }

  async fn wait(&self) {
    let mut count = self.count.lock().await;
    while *count > 0 {
      count = self.condvar.wait(count).await;
    }
  }
}
