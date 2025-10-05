use alloc::vec::Vec;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};

use crate::collections::queue_arc::ArcRingQueue;

pub type ArcLocalPriorityQueue<E> = ArcPriorityQueue<E, NoopRawMutex>;
pub type ArcCsPriorityQueue<E> = ArcPriorityQueue<E, CriticalSectionRawMutex>;

#[derive(Debug, Clone)]
pub struct ArcPriorityQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  queues: Vec<ArcRingQueue<E, RM>>,
}

impl<E, RM> ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  pub fn new(capacity_per_level: usize) -> Self {
    let mut queues = Vec::with_capacity(PRIORITY_LEVELS);
    for _ in 0..PRIORITY_LEVELS {
      queues.push(ArcRingQueue::new(capacity_per_level));
    }
    Self { queues }
  }

  pub fn set_dynamic(&self, dynamic: bool) {
    for queue in &self.queues {
      queue.set_dynamic(dynamic);
    }
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  fn priority_index(&self, element: &E) -> usize {
    element
      .get_priority()
      .unwrap_or(DEFAULT_PRIORITY)
      .clamp(0, PRIORITY_LEVELS as i8 - 1) as usize
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    let idx = self.priority_index(&element);
    self.queues[idx].offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    for queue in self.queues.iter().rev() {
      if let Some(item) = queue.poll_shared()? {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  pub fn len_shared(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.queues {
      match queue.len_shared() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }

  pub fn clean_up_shared(&self) {
    for queue in &self.queues {
      queue.clean_up_shared();
    }
  }
}

impl<E, RM> QueueBase<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.queues {
      match queue.capacity_shared() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }
}

impl<E, RM> QueueWriter<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E, RM> QueueReader<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
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
    queue.offer_shared(Msg(1, 0)).unwrap();
    queue.offer_shared(Msg(9, 7)).unwrap();
    queue.offer_shared(Msg(5, 3)).unwrap();

    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 9);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 5);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 1);
    assert!(queue.poll_shared().unwrap().is_none());
  }

  #[test]
  fn arc_priority_queue_len_and_clean_up() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(1);
    queue.offer_shared(Msg(1, 0)).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));
    queue.clean_up_shared();
    assert_eq!(queue.len_shared(), QueueSize::limited(0));
  }

  #[test]
  fn arc_priority_queue_len_across_levels() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2);
    queue.offer_shared(Msg(1, 0)).unwrap();
    queue.offer_shared(Msg(2, 5)).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(2));
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
    queue.offer_shared(OptionalPriority(1, Some(127))).unwrap();
    queue.offer_shared(OptionalPriority(2, Some(-128))).unwrap();
    queue.offer_shared(OptionalPriority(3, None)).unwrap();

    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 1);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 2);
  }
}
