use alloc::{collections::VecDeque, vec::Vec};
use core::{
  task::{Context, Poll, Waker},
  time::Duration,
};

use nexus_utils_core_rs::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, TimerDeadline,
};

#[derive(Debug)]
struct Entry<Item> {
  key: DeadlineTimerKey,
  remaining: Duration,
  item: Item,
}

/// Software-driven `DeadlineTimer` implementation for `no_std` environments.
///
/// Detects expiration by receiving elapsed time notifications via [`Self::advance`]
/// and queues expired items for retrieval in the next `poll_expired` call.
/// Enables handling of `ReceiveTimeout` without hardware timers while keeping the core abstraction unchanged.
#[derive(Debug)]
pub struct ManualDeadlineTimer<Item> {
  allocator: DeadlineTimerKeyAllocator,
  entries: Vec<Entry<Item>>,
  ready: VecDeque<DeadlineTimerExpired<Item>>,
  waker: Option<Waker>,
}

impl<Item> ManualDeadlineTimer<Item> {
  /// Creates a new timer with no entries.
  #[inline]
  pub fn new() -> Self {
    Self {
      allocator: DeadlineTimerKeyAllocator::new(),
      entries: Vec::new(),
      ready: VecDeque::new(),
      waker: None,
    }
  }

  fn wake(&mut self) {
    if let Some(waker) = self.waker.take() {
      waker.wake();
    }
  }

  /// Notifies elapsed time and queues expired items.
  pub fn advance(&mut self, elapsed: Duration) {
    if elapsed == Duration::ZERO && self.ready.is_empty() {
      return;
    }

    let mut idx = 0;
    let mut became_ready = false;

    while idx < self.entries.len() {
      if elapsed >= self.entries[idx].remaining {
        let entry = self.entries.swap_remove(idx);
        self.ready.push_back(DeadlineTimerExpired {
          key: entry.key,
          item: entry.item,
        });
        became_ready = true;
      } else if let Some(remaining) = self.entries[idx].remaining.checked_sub(elapsed) {
        self.entries[idx].remaining = remaining;
        idx += 1;
      } else {
        // If `checked_sub` returns None, treat as expired.
        let entry = self.entries.swap_remove(idx);
        self.ready.push_back(DeadlineTimerExpired {
          key: entry.key,
          item: entry.item,
        });
        became_ready = true;
      }
    }

    if became_ready {
      self.wake();
    }
  }

  fn push_ready(&mut self, key: DeadlineTimerKey, item: Item) {
    self.ready.push_back(DeadlineTimerExpired { key, item });
    self.wake();
  }
}

impl<Item> Default for ManualDeadlineTimer<Item> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Item> DeadlineTimer for ManualDeadlineTimer<Item> {
  type Error = DeadlineTimerError;
  type Item = Item;

  fn insert(&mut self, item: Self::Item, deadline: TimerDeadline) -> Result<DeadlineTimerKey, Self::Error> {
    let key = self.allocator.allocate();
    if deadline.as_duration() == Duration::ZERO {
      self.push_ready(key, item);
      return Ok(key);
    }

    self.entries.push(Entry {
      key,
      remaining: deadline.as_duration(),
      item,
    });
    Ok(key)
  }

  fn reset(&mut self, key: DeadlineTimerKey, deadline: TimerDeadline) -> Result<(), Self::Error> {
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
      entry.remaining = deadline.as_duration();
      return Ok(());
    }

    if let Some(position) = self.ready.iter().position(|expired| expired.key == key) {
      let expired = self.ready.remove(position).unwrap();
      self.entries.push(Entry {
        key,
        remaining: deadline.as_duration(),
        item: expired.item,
      });
      return Ok(());
    }

    Err(DeadlineTimerError::KeyNotFound)
  }

  fn cancel(&mut self, key: DeadlineTimerKey) -> Result<Option<Self::Item>, Self::Error> {
    if let Some(position) = self.entries.iter().position(|entry| entry.key == key) {
      let entry = self.entries.remove(position);
      return Ok(Some(entry.item));
    }

    if let Some(position) = self.ready.iter().position(|expired| expired.key == key) {
      let expired = self.ready.remove(position).unwrap();
      return Ok(Some(expired.item));
    }

    Ok(None)
  }

  fn poll_expired(&mut self, cx: &mut Context<'_>) -> Poll<Result<DeadlineTimerExpired<Self::Item>, Self::Error>> {
    if let Some(expired) = self.ready.pop_front() {
      return Poll::Ready(Ok(expired));
    }

    self.waker = Some(cx.waker().clone());
    Poll::Pending
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use core::task::Poll;
  use futures::task::noop_waker_ref;
  use std::{task::Context, time::Duration};

  #[test]
  fn manual_deadline_timer_expires_after_advance() {
    let mut queue = ManualDeadlineTimer::new();
    let key = queue
      .insert("timeout", TimerDeadline::from(Duration::from_millis(10)))
      .unwrap();

    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    assert!(matches!(queue.poll_expired(&mut cx), Poll::Pending));

    queue.advance(Duration::from_millis(5));
    assert!(matches!(queue.poll_expired(&mut cx), Poll::Pending));

    queue.advance(Duration::from_millis(5));
    let expired = queue.poll_expired(&mut cx).map(|res| res.unwrap());
    assert!(matches!(expired, Poll::Ready(exp) if exp.key == key && exp.item == "timeout"));
  }

  #[test]
  fn manual_deadline_timer_cancel_and_reset() {
    let mut queue = ManualDeadlineTimer::new();
    let key = queue
      .insert("value", TimerDeadline::from(Duration::from_millis(5)))
      .unwrap();

    let cancelled = queue.cancel(key).unwrap();
    assert_eq!(cancelled, Some("value"));
    assert!(queue.cancel(key).unwrap().is_none());

    let key = queue
      .insert("reset", TimerDeadline::from(Duration::from_millis(5)))
      .unwrap();
    queue.advance(Duration::from_millis(5));

    queue
      .reset(key, TimerDeadline::from(Duration::from_millis(10)))
      .unwrap();

    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    assert!(matches!(queue.poll_expired(&mut cx), Poll::Pending));
    queue.advance(Duration::from_millis(10));
    let expired = queue.poll_expired(&mut cx).map(|res| res.unwrap());
    assert!(matches!(expired, Poll::Ready(exp) if exp.key == key && exp.item == "reset"));
  }
}
