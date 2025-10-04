use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer, SharedRingQueue, DEFAULT_CAPACITY,
};

use crate::sync::{ArcShared, ArcStateCell};

#[derive(Debug, Clone)]
pub struct ArcRingQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: SharedRingQueue<ArcShared<ArcStateCell<RingBuffer<E>, RM>>, E>,
}

pub type ArcLocalRingQueue<E> = ArcRingQueue<E, NoopRawMutex>;

impl<E, RM> ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new(capacity: usize) -> Self {
    let storage = ArcShared::new(ArcStateCell::new(RingBuffer::new(capacity)));
    Self {
      inner: SharedRingQueue::new(storage),
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.inner = self.inner.with_dynamic(dynamic);
    self
  }

  pub fn set_dynamic(&self, dynamic: bool) {
    self.inner.set_dynamic(dynamic);
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_shared()
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
  }

  pub fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }

  pub fn capacity_shared(&self) -> QueueSize {
    self.inner.capacity_shared()
  }
}

impl<E, RM> QueueBase<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E, RM> QueueWriter<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E, RM> QueueReader<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E, RM> Default for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn arc_ring_queue_offer_poll() {
    let queue = ArcLocalRingQueue::new(1);
    queue.offer_shared(1).unwrap();
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
  }

  #[test]
  fn arc_ring_queue_shared_clone() {
    let queue = ArcLocalRingQueue::new(2);
    let cloned = queue.clone();

    queue.offer_shared(5).unwrap();
    cloned.offer_shared(6).unwrap();

    assert_eq!(queue.len_shared().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(5));
    assert_eq!(cloned.poll_shared().unwrap(), Some(6));
  }
}
