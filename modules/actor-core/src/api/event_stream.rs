use crate::FailureEventListener;

/// FailureEvent を外部へ配信するためのストリーム抽象。
///
/// 実装は `actor-std` や `actor-embedded` といった周辺クレート側に配置し、
/// `actor-core` からは依存逆転の形で利用する。
pub trait FailureEventStream: Clone + Send + Sync + 'static {
  /// 購読を表すハンドル型。Drop 時に購読解除などの後処理を担う。
  type Subscription: Send + 'static;

  /// FailureEvent の通知を受け取るためのリスナを返す。
  fn listener(&self) -> FailureEventListener;

  /// 新しい購読者を登録し、購読ハンドルを返す。
  fn subscribe(&self, listener: FailureEventListener) -> Self::Subscription;
}

#[cfg(all(test, feature = "std"))]
pub(crate) mod tests {
  use super::FailureEventStream;
  use crate::FailureEvent;
  use crate::FailureEventListener;
  use alloc::sync::Arc;
  use core::sync::atomic::{AtomicU64, Ordering};
  use std::sync::Mutex;

  /// テスト専用のインメモリ実装。
  #[derive(Clone, Default)]
  pub(crate) struct TestFailureEventStream {
    inner: Arc<TestFailureEventStreamInner>,
  }

  #[derive(Default)]
  struct TestFailureEventStreamInner {
    next_id: AtomicU64,
    listeners: Mutex<Vec<(u64, FailureEventListener)>>,
  }

  #[derive(Clone)]
  pub(crate) struct TestFailureEventSubscription {
    inner: Arc<TestFailureEventStreamInner>,
    id: u64,
  }

  impl FailureEventStream for TestFailureEventStream {
    type Subscription = TestFailureEventSubscription;

    fn listener(&self) -> FailureEventListener {
      let inner = self.inner.clone();
      Arc::new(move |event: FailureEvent| {
        let snapshot: Vec<FailureEventListener> = {
          let guard = inner.listeners.lock().unwrap();
          guard.iter().map(|(_, listener)| listener.clone()).collect()
        };
        for listener in snapshot.into_iter() {
          listener(event.clone());
        }
      })
    }

    fn subscribe(&self, listener: FailureEventListener) -> Self::Subscription {
      let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
      {
        let mut guard = self.inner.listeners.lock().unwrap();
        guard.push((id, listener));
      }
      TestFailureEventSubscription {
        inner: self.inner.clone(),
        id,
      }
    }
  }

  impl Drop for TestFailureEventSubscription {
    fn drop(&mut self) {
      if let Ok(mut guard) = self.inner.listeners.lock() {
        if let Some(index) = guard.iter().position(|(slot_id, _)| *slot_id == self.id) {
          guard.swap_remove(index);
        }
      }
    }
  }
}
