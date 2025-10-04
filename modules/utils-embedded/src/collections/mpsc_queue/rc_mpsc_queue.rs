use alloc::collections::VecDeque;
use alloc::rc::Rc;
use core::cell::RefCell;

use nexus_utils_core_rs::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue};

#[derive(Debug)]
struct RcMpscQueueState<E> {
  buffer: VecDeque<E>,
  closed: bool,
}

impl<E> RcMpscQueueState<E> {
  fn new() -> Self {
    Self {
      buffer: VecDeque::new(),
      closed: false,
    }
  }

  fn len(&self) -> usize {
    self.buffer.len()
  }
}

#[derive(Debug, Clone)]
pub struct RcMpscUnboundedQueue<E> {
  state: Rc<RefCell<RcMpscQueueState<E>>>,
}

impl<E> RcMpscUnboundedQueue<E> {
  pub fn new() -> Self {
    Self {
      state: Rc::new(RefCell::new(RcMpscQueueState::new())),
    }
  }

  fn with_state<R>(&self, f: impl FnOnce(&mut RcMpscQueueState<E>) -> R) -> R {
    let mut guard = self.state.borrow_mut();
    f(&mut guard)
  }

  fn with_state_ref<R>(&self, f: impl FnOnce(&RcMpscQueueState<E>) -> R) -> R {
    let guard = self.state.borrow();
    f(&guard)
  }
}

impl<E: Element> RcMpscUnboundedQueue<E> {
  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.with_state(|state| {
      if state.closed {
        return Err(QueueError::Closed(element));
      }
      state.buffer.push_back(element);
      Ok(())
    })
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.with_state(|state| {
      if let Some(item) = state.buffer.pop_front() {
        return Ok(Some(item));
      }
      if state.closed {
        Err(QueueError::Disconnected)
      } else {
        Ok(None)
      }
    })
  }

  pub fn clean_up_shared(&self) {
    self.with_state(|state| {
      state.buffer.clear();
      state.closed = true;
    });
  }

  pub fn len_shared(&self) -> QueueSize {
    self.with_state_ref(|state| QueueSize::limited(state.len()))
  }
}

impl<E: Element> QueueBase<E> for RcMpscUnboundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }
}

impl<E: Element> QueueWriter<E> for RcMpscUnboundedQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscUnboundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<E: Element> SharedQueue<E> for RcMpscUnboundedQueue<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    RcMpscUnboundedQueue::offer_shared(self, element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    RcMpscUnboundedQueue::poll_shared(self)
  }

  fn clean_up_shared(&self) {
    RcMpscUnboundedQueue::clean_up_shared(self)
  }
}

impl<E> Default for RcMpscUnboundedQueue<E> {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Debug, Clone)]
pub struct RcMpscBoundedQueue<E> {
  state: Rc<RefCell<RcMpscQueueState<E>>>,
  capacity: usize,
}

impl<E> RcMpscBoundedQueue<E> {
  pub fn new(capacity: usize) -> Self {
    Self {
      state: Rc::new(RefCell::new(RcMpscQueueState::new())),
      capacity,
    }
  }

  fn with_state<R>(&self, f: impl FnOnce(&mut RcMpscQueueState<E>) -> R) -> R {
    let mut guard = self.state.borrow_mut();
    f(&mut guard)
  }

  fn with_state_ref<R>(&self, f: impl FnOnce(&RcMpscQueueState<E>) -> R) -> R {
    let guard = self.state.borrow();
    f(&guard)
  }
}

impl<E: Element> RcMpscBoundedQueue<E> {
  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.with_state(|state| {
      if state.closed {
        return Err(QueueError::Closed(element));
      }
      if state.buffer.len() >= self.capacity {
        return Err(QueueError::Full(element));
      }
      state.buffer.push_back(element);
      Ok(())
    })
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.with_state(|state| {
      if let Some(item) = state.buffer.pop_front() {
        return Ok(Some(item));
      }
      if state.closed {
        Err(QueueError::Disconnected)
      } else {
        Ok(None)
      }
    })
  }

  pub fn clean_up_shared(&self) {
    self.with_state(|state| {
      state.buffer.clear();
      state.closed = true;
    });
  }

  pub fn len_shared(&self) -> QueueSize {
    self.with_state_ref(|state| QueueSize::limited(state.len()))
  }
}

impl<E: Element> QueueBase<E> for RcMpscBoundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limited(self.capacity)
  }
}

impl<E: Element> QueueWriter<E> for RcMpscBoundedQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscBoundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<E: Element> SharedQueue<E> for RcMpscBoundedQueue<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    RcMpscBoundedQueue::offer_shared(self, element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    RcMpscBoundedQueue::poll_shared(self)
  }

  fn clean_up_shared(&self) {
    RcMpscBoundedQueue::clean_up_shared(self)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_unbounded_offer_poll() {
    let queue: RcMpscUnboundedQueue<u32> = RcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn rc_bounded_capacity_limit() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(1);
    queue.offer_shared(42).unwrap();
    let err = queue.offer_shared(99).unwrap_err();
    assert!(matches!(err, QueueError::Full(99)));
  }
}
