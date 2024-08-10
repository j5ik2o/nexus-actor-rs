use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Clone)]
pub struct WaitGroup {
  inner: Arc<Inner>,
}

struct Inner {
  count: Mutex<usize>,
  notify: Notify,
}

impl WaitGroup {
  pub fn new() -> Self {
    WaitGroup {
      inner: Arc::new(Inner {
        count: Mutex::new(0),
        notify: Notify::new(),
      }),
    }
  }

  pub async fn add(&self, n: usize) {
    let mut count = self.inner.count.lock().await;
    *count += n;
  }

  pub async fn done(&self) {
    let mut count = self.inner.count.lock().await;
    tracing::debug!("done: count={}", *count);
    *count -= 1;
    if *count == 0 {
      self.inner.notify.notify_waiters();
    }
  }

  pub async fn wait(&self) {
    loop {
      let count = self.inner.count.lock().await;
      if *count == 0 {
        break;
      }
      drop(count);
      self.inner.notify.notified().await;
    }
  }
}

// 使用例
#[tokio::test]
async fn test_main() {
  let wg = WaitGroup::new();

  for i in 0..3 {
    let wg = wg.clone();
    tokio::spawn(async move {
      // 非同期の作業をシミュレート
      tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
      println!("Task {} completed", i);
      wg.done().await;
    });
  }

  wg.add(3).await;
  wg.wait().await;
  println!("All tasks completed");
}
