use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedPriorityQueue, SharedQueue,
  PRIORITY_LEVELS,
};

use crate::collections::queue_arc::ArcRingQueue;

pub type ArcLocalPriorityQueue<E> = ArcPriorityQueue<E, NoopRawMutex>;
pub type ArcCsPriorityQueue<E> = ArcPriorityQueue<E, CriticalSectionRawMutex>;

#[derive(Debug, Clone)]
pub struct ArcPriorityQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: SharedPriorityQueue<ArcRingQueue<E, RM>, E>,
}

impl<E, RM> ArcPriorityQueue<E, RM>
where
  RM: RawMutex,
{
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| ArcRingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: SharedPriorityQueue::new(levels),
    }
  }

  pub fn set_dynamic(&self, dynamic: bool) {
    for queue in self.inner.levels() {
      queue.set_dynamic(dynamic);
    }
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  pub fn levels(&self) -> &[ArcRingQueue<E, RM>] {
    self.inner.levels()
  }

  pub fn levels_mut(&mut self) -> &mut [ArcRingQueue<E, RM>] {
    self.inner.levels_mut()
  }

  pub fn inner(&self) -> &SharedPriorityQueue<ArcRingQueue<E, RM>, E> {
    &self.inner
  }
}

impl<E, RM> QueueBase<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E, RM> QueueWriter<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E, RM> QueueReader<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E, RM> SharedQueue<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;
  use nexus_utils_core_rs::{QueueBase, QueueReader, QueueWriter, SharedQueue};

  fn prepare() {
    init_arc_critical_section();
  }

  #[derive(Debug, Clone)]
  struct Msg(i32, i8);

  impl nexus_utils_core_rs::Element for Msg {}

  impl PriorityMessage for Msg {
    fn get_priority(&self) -> Option<i8> {
      Some(self.1)
    }
  }

  #[test]
  fn arc_priority_queue_orders() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(3);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(9, 7)).unwrap();
    queue.offer(Msg(5, 3)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 9);
    assert_eq!(queue.poll().unwrap().unwrap().0, 5);
    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn arc_priority_queue_len_and_clean_up() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(1);
    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn arc_priority_queue_len_across_levels() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(2, 5)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(2));
  }

  #[test]
  fn arc_priority_queue_capacity_behaviour() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2);
    assert!(queue.capacity().is_limitless());

    queue.set_dynamic(false);
    let expected = QueueSize::limited(2 * PRIORITY_LEVELS);
    assert_eq!(queue.capacity(), expected);
  }

  #[test]
  fn arc_priority_queue_trait_cleanup() {
    prepare();
    let mut queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2).with_dynamic(false);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(2, 1)).unwrap();
    assert_eq!(queue.poll().unwrap().unwrap().0, 2);
    queue.clean_up();
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn arc_priority_queue_priority_clamp_and_default() {
    prepare();
    #[derive(Debug, Clone)]
    struct OptionalPriority(i32, Option<i8>);

    impl nexus_utils_core_rs::Element for OptionalPriority {}

    impl PriorityMessage for OptionalPriority {
      fn get_priority(&self) -> Option<i8> {
        self.1
      }
    }

    let queue: ArcLocalPriorityQueue<OptionalPriority> = ArcLocalPriorityQueue::new(1).with_dynamic(false);
    queue.offer(OptionalPriority(1, Some(127))).unwrap();
    queue.offer(OptionalPriority(2, Some(-128))).unwrap();
    queue.offer(OptionalPriority(3, None)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert_eq!(queue.poll().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll().unwrap().unwrap().0, 2);
  }
}
