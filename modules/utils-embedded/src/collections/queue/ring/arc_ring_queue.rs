use embassy_sync::blocking_mutex::raw::{NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer, SharedQueue, SharedRingQueue,
  DEFAULT_CAPACITY,
};

use crate::sync::{ArcShared, ArcStateCell};

#[derive(Debug)]
pub struct ArcRingQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex,
{
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
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E, RM> QueueReader<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E, RM> SharedQueue<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&self) {
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

impl<E, RM> Clone for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;

  fn prepare() {
    init_arc_critical_section();
  }

  #[test]
  fn arc_ring_queue_offer_poll() {
    prepare();
    let queue = ArcLocalRingQueue::new(1);
    queue.offer(1).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(1));
  }

  #[test]
  fn arc_ring_queue_shared_clone() {
    prepare();
    let queue = ArcLocalRingQueue::new(2);
    let cloned = queue.clone();

    queue.offer(5).unwrap();
    cloned.offer(6).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll().unwrap(), Some(5));
    assert_eq!(cloned.poll().unwrap(), Some(6));
  }

  #[test]
  fn arc_ring_queue_dynamic_and_clean_up() {
    prepare();
    let queue = ArcLocalRingQueue::new(1).with_dynamic(false);
    queue.offer(1).unwrap();
    assert!(matches!(queue.offer(2), Err(QueueError::Full(2))));

    queue.clean_up();
    assert_eq!(queue.len().to_usize(), 0);
  }

  #[test]
  fn arc_ring_queue_capacity_reporting() {
    prepare();
    let queue: ArcRingQueue<u32> = ArcLocalRingQueue::new(2);
    assert!(queue.capacity().is_limitless());

    queue.set_dynamic(false);
    assert_eq!(queue.capacity(), QueueSize::limited(2));
  }

  #[test]
  fn arc_ring_queue_trait_interface() {
    prepare();
    let mut queue: ArcRingQueue<u32> = ArcLocalRingQueue::new(1).with_dynamic(false);
    queue.offer(4).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(4));
  }
}
