use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nexus_utils_core_rs::{async_trait, WaitGroup as CoreWaitGroup, WaitGroupBackend};
use tokio::sync::Notify;

/// Tokioランタイムを使用したWaitGroupバックエンド実装
///
/// 非同期タスクの同期に使用され、複数の非同期操作が完了するまで待機できます。
#[derive(Clone)]
pub struct TokioWaitGroupBackend {
  inner: Arc<Inner>,
}

struct Inner {
  count: AtomicUsize,
  notify: Notify,
}

#[async_trait(?Send)]
impl WaitGroupBackend for TokioWaitGroupBackend {
  fn new() -> Self {
    Self::with_count(0)
  }

  fn with_count(count: usize) -> Self {
    Self {
      inner: Arc::new(Inner {
        count: AtomicUsize::new(count),
        notify: Notify::new(),
      }),
    }
  }

  fn add(&self, n: usize) {
    self.inner.count.fetch_add(n, Ordering::SeqCst);
  }

  fn done(&self) {
    let prev = self.inner.count.fetch_sub(1, Ordering::SeqCst);
    assert!(prev > 0, "WaitGroup::done called more times than add");
    if prev == 1 {
      self.inner.notify.notify_waiters();
    }
  }

  async fn wait(&self) {
    let inner = self.inner.clone();
    loop {
      if inner.count.load(Ordering::SeqCst) == 0 {
        return;
      }
      inner.notify.notified().await;
    }
  }
}

/// Tokioバックエンドを使用するWaitGroupの型エイリアス
///
/// 複数の非同期タスクが完了するまで待機するための同期プリミティブです。
pub type WaitGroup = CoreWaitGroup<TokioWaitGroupBackend>;

#[cfg(test)]
mod tests {
  use super::WaitGroup;
  use tokio::join;

  #[tokio::test]
  async fn wait_group_completes() {
    let wg = WaitGroup::new();
    wg.add(2);
    let worker_wg = wg.clone();

    let wait_fut = wg.wait();
    let worker = async move {
      worker_wg.done();
      worker_wg.done();
    };

    join!(worker, wait_fut);
  }
}
