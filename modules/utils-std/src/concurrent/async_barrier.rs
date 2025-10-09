use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nexus_utils_core_rs::{async_trait, AsyncBarrier as CoreAsyncBarrier, AsyncBarrierBackend};
use tokio::sync::Notify;

/// Tokioランタイムを使用した非同期バリアのバックエンド実装
///
/// 複数のタスクが互いに待ち合わせるための同期プリミティブです。
/// 指定された数のタスクが`wait()`を呼び出すまで、すべてのタスクをブロックします。
#[derive(Clone)]
pub struct TokioAsyncBarrierBackend {
  inner: Arc<Inner>,
}

struct Inner {
  remaining: AtomicUsize,
  initial: usize,
  notify: Notify,
}

#[async_trait(?Send)]
impl AsyncBarrierBackend for TokioAsyncBarrierBackend {
  fn new(count: usize) -> Self {
    assert!(count > 0, "AsyncBarrier must have positive count");
    Self {
      inner: Arc::new(Inner {
        remaining: AtomicUsize::new(count),
        initial: count,
        notify: Notify::new(),
      }),
    }
  }

  async fn wait(&self) {
    let inner = self.inner.clone();
    let prev = inner.remaining.fetch_sub(1, Ordering::SeqCst);
    assert!(prev > 0, "AsyncBarrier::wait called more times than count");
    if prev == 1 {
      inner.remaining.store(inner.initial, Ordering::SeqCst);
      inner.notify.notify_waiters();
    } else {
      loop {
        if inner.remaining.load(Ordering::SeqCst) == inner.initial {
          break;
        }
        inner.notify.notified().await;
      }
    }
  }
}

/// Tokioランタイムを使用した非同期バリア
///
/// 指定された数のタスクが到達するまで、すべてのタスクを待機させる同期プリミティブです。
/// すべてのタスクがバリアに到達すると、バリアは再利用可能な状態にリセットされます。
pub type AsyncBarrier = CoreAsyncBarrier<TokioAsyncBarrierBackend>;

#[cfg(test)]
mod tests {
  use super::AsyncBarrier;
  use tokio::join;

  #[tokio::test]
  async fn barrier_releases_all() {
    let barrier = AsyncBarrier::new(2);
    let b2 = barrier.clone();

    let first = async move {
      barrier.wait().await;
    };
    let second = async move {
      b2.wait().await;
    };

    join!(first, second);
  }
}
