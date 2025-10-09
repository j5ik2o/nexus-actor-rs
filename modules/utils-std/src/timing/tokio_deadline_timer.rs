use std::collections::HashMap;

use core::task::Poll;

use nexus_utils_core_rs::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, TimerDeadline,
};
use tokio_util::time::delay_queue::{DelayQueue as InnerDelayQueue, Key as InnerKey};

/// `tokio_util::time::DelayQueue` をラップし、`DeadlineTimer` 抽象を満たす実装。
#[derive(Debug)]
pub struct TokioDeadlineTimer<Item> {
  inner: InnerDelayQueue<Item>,
  allocator: DeadlineTimerKeyAllocator,
  forward: HashMap<DeadlineTimerKey, InnerKey>,
  reverse: HashMap<InnerKey, DeadlineTimerKey>,
}

impl<Item> TokioDeadlineTimer<Item> {
  /// 空の DeadlineTimer を生成する。
  #[inline]
  pub fn new() -> Self {
    Self::with_inner(InnerDelayQueue::new())
  }

  /// 予め容量を確保した DeadlineTimer を生成する。
  #[inline]
  pub fn with_capacity(capacity: usize) -> Self {
    Self::with_inner(InnerDelayQueue::with_capacity(capacity))
  }

  fn with_inner(inner: InnerDelayQueue<Item>) -> Self {
    Self {
      inner,
      allocator: DeadlineTimerKeyAllocator::new(),
      forward: HashMap::new(),
      reverse: HashMap::new(),
    }
  }

  fn release_key_mapping(&mut self, inner_key: &InnerKey) -> Option<DeadlineTimerKey> {
    if let Some(key) = self.reverse.remove(inner_key) {
      self.forward.remove(&key);
      Some(key)
    } else {
      None
    }
  }
}

impl<Item> Default for TokioDeadlineTimer<Item> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Item> DeadlineTimer for TokioDeadlineTimer<Item> {
  type Error = DeadlineTimerError;
  type Item = Item;

  fn insert(&mut self, item: Self::Item, deadline: TimerDeadline) -> Result<DeadlineTimerKey, Self::Error> {
    let key = self.allocator.allocate();
    let inner_key = self.inner.insert(item, deadline.as_duration());
    self.forward.insert(key, inner_key);
    self.reverse.insert(inner_key, key);
    Ok(key)
  }

  fn reset(&mut self, key: DeadlineTimerKey, deadline: TimerDeadline) -> Result<(), Self::Error> {
    let inner_key = *self.forward.get(&key).ok_or(DeadlineTimerError::KeyNotFound)?;
    self.inner.reset(&inner_key, deadline.as_duration());
    Ok(())
  }

  fn cancel(&mut self, key: DeadlineTimerKey) -> Result<Option<Self::Item>, Self::Error> {
    if let Some(inner_key) = self.forward.remove(&key) {
      self.reverse.remove(&inner_key);
      let removed = self.inner.remove(&inner_key);
      Ok(Some(removed.into_inner()))
    } else {
      Ok(None)
    }
  }

  fn poll_expired(
    &mut self,
    cx: &mut core::task::Context<'_>,
  ) -> Poll<Result<DeadlineTimerExpired<Self::Item>, Self::Error>> {
    match self.inner.poll_expired(cx) {
      Poll::Ready(Some(expired)) => {
        let inner_key = expired.key();
        let item = expired.into_inner();
        if let Some(key) = self.release_key_mapping(&inner_key) {
          Poll::Ready(Ok(DeadlineTimerExpired { key, item }))
        } else {
          Poll::Ready(Err(DeadlineTimerError::KeyNotFound))
        }
      }
      Poll::Ready(None) => Poll::Ready(Err(DeadlineTimerError::Closed)),
      Poll::Pending => Poll::Pending,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::task::noop_waker_ref;
  use std::{task::Context, time::Duration};

  #[tokio::test(start_paused = true)]
  async fn tokio_deadline_timer_expires_after_duration() {
    let mut queue = TokioDeadlineTimer::new();
    let key = queue
      .insert("hello", TimerDeadline::from(Duration::from_millis(10)))
      .unwrap();

    tokio::time::advance(Duration::from_millis(10)).await;

    let expired = futures::future::poll_fn(|cx| queue.poll_expired(cx))
      .await
      .expect("expired");
    assert_eq!(expired.item, "hello");
    assert_eq!(expired.key, key);
  }

  #[tokio::test(start_paused = true)]
  async fn tokio_deadline_timer_reset_extends_deadline() {
    let mut queue = TokioDeadlineTimer::new();
    let key = queue
      .insert("value", TimerDeadline::from(Duration::from_secs(1)))
      .unwrap();
    queue.reset(key, TimerDeadline::from(Duration::from_secs(2))).unwrap();

    tokio::time::advance(Duration::from_secs(1)).await;

    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    assert!(matches!(queue.poll_expired(&mut cx), Poll::Pending));

    tokio::time::advance(Duration::from_secs(1)).await;

    let expired = futures::future::poll_fn(|cx| queue.poll_expired(cx))
      .await
      .expect("expired");
    assert_eq!(expired.key, key);
    assert_eq!(expired.item, "value");
  }

  #[tokio::test(flavor = "current_thread")]
  async fn tokio_deadline_timer_cancel_returns_value() {
    let mut queue = TokioDeadlineTimer::new();
    let key = queue
      .insert("cancel", TimerDeadline::from(Duration::from_millis(5)))
      .unwrap();
    let removed = queue.cancel(key).unwrap();
    assert_eq!(removed, Some("cancel"));
    assert!(queue.cancel(key).unwrap().is_none());
  }
}
