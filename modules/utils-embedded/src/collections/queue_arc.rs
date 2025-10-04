use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use embassy_sync::mutex::Mutex;
use nexus_utils_core_rs::{
  collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer},
  DEFAULT_CAPACITY,
};

#[derive(Debug, Clone)]
pub struct ArcRingQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: Arc<Mutex<RM, RingBuffer<E>>>,
}

pub type ArcLocalRingQueue<E> = ArcRingQueue<E, NoopRawMutex>;

impl<E, RM> ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new(capacity: usize) -> Self {
    Self {
      inner: Arc::new(Mutex::new(RingBuffer::new(capacity))),
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  pub fn set_dynamic(&mut self, dynamic: bool) {
    self.with_lock(|buffer| buffer.set_dynamic(dynamic));
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.with_lock(|buffer| buffer.offer(element))
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.with_lock(|buffer| buffer.poll())
  }

  pub fn len_shared(&self) -> QueueSize {
    self.with_lock(|buffer| buffer.len())
  }

  pub fn clean_up_shared(&self) {
    self.with_lock(|buffer| buffer.clean_up());
  }

  fn with_lock<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
    let mut guard = self
      .inner
      .try_lock()
      .unwrap_or_else(|_| panic!("ring queue lock contention"));
    f(&mut guard)
  }
}

impl<E, RM> QueueBase<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.with_lock(|buffer| buffer.capacity())
  }
}

impl<E, RM> QueueWriter<E> for ArcRingQueue<E, RM>
where
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E, RM> QueueReader<E> for ArcRingQueue<E, RM>
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

  #[cfg(feature = "arc")]
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
