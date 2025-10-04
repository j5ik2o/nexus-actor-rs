#[cfg(test)]
use crate::runtime::TokioCoreSpawner;
#[cfg(test)]
use nexus_actor_core_rs::runtime::{CoreSpawner, CoreTaskFuture};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct WaitGroup {
  inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
  count: AtomicUsize,
  notify: Notify,
}

impl WaitGroup {
  pub fn new() -> Self {
    WaitGroup {
      inner: Arc::new(Inner {
        count: AtomicUsize::new(0),
        notify: Notify::new(),
      }),
    }
  }

  pub fn with_count(count: usize) -> Self {
    WaitGroup {
      inner: Arc::new(Inner {
        count: AtomicUsize::new(count),
        notify: Notify::new(),
      }),
    }
  }

  pub fn add(&self, n: usize) {
    self.inner.count.fetch_add(n, Ordering::SeqCst);
  }

  pub fn done(&self) {
    let previous = self.inner.count.fetch_sub(1, Ordering::SeqCst);
    tracing::debug!("done: count={}", previous);
    assert!(previous > 0, "WaitGroup::done called more times than add");
    if previous == 1 {
      self.inner.notify.notify_waiters();
    }
  }

  pub async fn wait(&self) {
    while self.inner.count.load(Ordering::SeqCst) != 0 {
      let notified = self.inner.notify.notified();
      if self.inner.count.load(Ordering::SeqCst) == 0 {
        break;
      }
      notified.await;
    }
  }
}

impl Default for WaitGroup {
  fn default() -> Self {
    Self::new()
  }
}

// 使用例
#[tokio::test]
async fn test_main() {
  let wg = WaitGroup::new();

  for i in 0..3 {
    let wg = wg.clone();
    let task: CoreTaskFuture = Box::pin(async move {
      tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
      tracing::info!("Task {} completed", i);
      wg.done();
    });
    TokioCoreSpawner::current()
      .spawn(task)
      .expect("spawn wait group task")
      .detach();
  }

  wg.add(3);
  wg.wait().await;
  tracing::debug!("All tasks completed");
}
