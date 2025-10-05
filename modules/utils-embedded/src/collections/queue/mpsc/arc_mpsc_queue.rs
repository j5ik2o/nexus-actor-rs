use crate::sync::{ArcShared, ArcStateCell};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  Element, MpscBuffer, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedMpscQueue, SharedQueue,
};

#[derive(Debug)]
pub struct ArcMpscUnboundedQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: SharedMpscQueue<ArcShared<ArcStateCell<MpscBuffer<E>, RM>>, E>,
}

pub type ArcLocalMpscUnboundedQueue<E> = ArcMpscUnboundedQueue<E, NoopRawMutex>;
pub type ArcCsMpscUnboundedQueue<E> = ArcMpscUnboundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new() -> Self {
    let storage = ArcShared::new(ArcStateCell::new(MpscBuffer::new(None)));
    Self {
      inner: SharedMpscQueue::new(storage),
    }
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: Element, {
    self.inner.offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: Element, {
    self.inner.poll_shared()
  }

  pub fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
  }
}

impl<E, RM> QueueBase<E> for ArcMpscUnboundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E, RM> QueueWriter<E> for ArcMpscUnboundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E, RM> QueueReader<E> for ArcMpscUnboundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E, RM> SharedQueue<E> for ArcMpscUnboundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up_shared(&self) {
    self.clean_up_shared();
  }
}

#[derive(Debug)]
pub struct ArcMpscBoundedQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: SharedMpscQueue<ArcShared<ArcStateCell<MpscBuffer<E>, RM>>, E>,
}

pub type ArcLocalMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, NoopRawMutex>;
pub type ArcCsMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new(capacity: usize) -> Self {
    let storage = ArcShared::new(ArcStateCell::new(MpscBuffer::new(Some(capacity))));
    Self {
      inner: SharedMpscQueue::new(storage),
    }
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: Element, {
    self.inner.offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: Element, {
    self.inner.poll_shared()
  }

  pub fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
  }

  pub fn capacity_shared(&self) -> QueueSize {
    self.inner.capacity_shared()
  }
}

impl<E, RM> QueueBase<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E, RM> QueueWriter<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E, RM> QueueReader<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E, RM> SharedQueue<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up_shared(&self) {
    self.clean_up_shared();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;
  use nexus_utils_core_rs::{QueueBase, QueueReader, QueueWriter};

  fn prepare() {
    init_arc_critical_section();
  }

  #[test]
  fn arc_unbounded_offer_poll() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn arc_unbounded_clean_up_signals_disconnect() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u8> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(9).unwrap();
    queue.clean_up_shared();

    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(1), Err(QueueError::Closed(1))));
  }

  #[test]
  fn arc_bounded_capacity_limit() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(1);
    queue.offer_shared(10).unwrap();
    let err = queue.offer_shared(11).unwrap_err();
    assert!(matches!(err, QueueError::Full(11)));
  }

  #[test]
  fn arc_bounded_clean_up_closes_queue() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(2);
    queue.offer_shared(1).unwrap();
    queue.clean_up_shared();

    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(2), Err(QueueError::Closed(2))));
  }

  #[test]
  fn arc_bounded_reports_len_and_capacity() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(3);
    assert_eq!(queue.capacity_shared(), QueueSize::limited(3));

    queue.offer_shared(1).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));
  }

  #[test]
  fn arc_unbounded_offer_poll_via_traits() {
    prepare();
    let mut queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer(7).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(7));
  }

  #[test]
  fn arc_unbounded_capacity_reports_limitless() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    assert!(queue.capacity().is_limitless());
  }

  #[test]
  fn arc_bounded_trait_cleanup_marks_closed() {
    prepare();
    let mut queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(3), Err(QueueError::Closed(3))));
  }
}
