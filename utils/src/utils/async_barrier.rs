use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Clone)]
pub struct AsyncBarrier {
  notify: Arc<Notify>,
  count: Arc<Mutex<usize>>,
}

impl AsyncBarrier {
  pub fn new(count: usize) -> Self {
    AsyncBarrier {
      notify: Arc::new(Notify::new()),
      count: Arc::new(Mutex::new(count)),
    }
  }

  pub async fn wait(&self) {
    let mut count = self.count.lock().await;
    *count -= 1;
    if *count == 0 {
      self.notify.notify_waiters();
    } else {
      drop(count);
      self.notify.notified().await;
    }
  }
}
