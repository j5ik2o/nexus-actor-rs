use alloc::collections::VecDeque;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};

use nexus_utils_core_rs::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue};

use crate::sync::ArcStateCell;

#[derive(Debug)]
struct ArcMpscQueueState<E> {
  buffer: VecDeque<E>,
  closed: bool,
}

impl<E> ArcMpscQueueState<E> {
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
pub struct ArcMpscUnboundedQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  state: ArcStateCell<ArcMpscQueueState<E>, RM>,
}

pub type ArcLocalMpscUnboundedQueue<E> = ArcMpscUnboundedQueue<E, NoopRawMutex>;
pub type ArcCsMpscUnboundedQueue<E> = ArcMpscUnboundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new() -> Self {
    Self {
      state: ArcStateCell::new(ArcMpscQueueState::new()),
    }
  }

  fn with_state<R>(&self, f: impl FnOnce(&mut ArcMpscQueueState<E>) -> R) -> R {
    let mut guard = self.state.borrow_mut();
    f(&mut guard)
  }

  fn with_state_ref<R>(&self, f: impl FnOnce(&ArcMpscQueueState<E>) -> R) -> R {
    let guard = self.state.borrow();
    f(&guard)
  }
}

impl<E: Element, RM> ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
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

impl<E: Element, RM> QueueBase<E> for ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }
}

impl<E: Element, RM> QueueWriter<E> for ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E: Element, RM> QueueReader<E> for ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<E: Element, RM> SharedQueue<E> for ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    ArcMpscUnboundedQueue::offer_shared(self, element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    ArcMpscUnboundedQueue::poll_shared(self)
  }

  fn clean_up_shared(&self) {
    ArcMpscUnboundedQueue::clean_up_shared(self)
  }
}

#[derive(Debug, Clone)]
pub struct ArcMpscBoundedQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  state: ArcStateCell<ArcMpscQueueState<E>, RM>,
  capacity: usize,
}

pub type ArcLocalMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, NoopRawMutex>;
pub type ArcCsMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new(capacity: usize) -> Self {
    Self {
      state: ArcStateCell::new(ArcMpscQueueState::new()),
      capacity,
    }
  }

  fn with_state<R>(&self, f: impl FnOnce(&mut ArcMpscQueueState<E>) -> R) -> R {
    let mut guard = self.state.borrow_mut();
    f(&mut guard)
  }

  fn with_state_ref<R>(&self, f: impl FnOnce(&ArcMpscQueueState<E>) -> R) -> R {
    let guard = self.state.borrow();
    f(&guard)
  }
}

impl<E: Element, RM> ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
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

  pub fn capacity_shared(&self) -> QueueSize {
    QueueSize::limited(self.capacity)
  }
}

impl<E: Element, RM> QueueBase<E> for ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity_shared()
  }
}

impl<E: Element, RM> QueueWriter<E> for ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E: Element, RM> QueueReader<E> for ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<E: Element, RM> SharedQueue<E> for ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    ArcMpscBoundedQueue::offer_shared(self, element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    ArcMpscBoundedQueue::poll_shared(self)
  }

  fn clean_up_shared(&self) {
    ArcMpscBoundedQueue::clean_up_shared(self)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn arc_unbounded_offer_poll() {
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn arc_bounded_capacity_limit() {
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(1);
    queue.offer_shared(10).unwrap();
    let err = queue.offer_shared(11).unwrap_err();
    assert!(matches!(err, QueueError::Full(11)));
  }
}
