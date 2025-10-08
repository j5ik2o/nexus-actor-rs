use core::marker::PhantomData;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};

use nexus_actor_core_rs::{
  Mailbox, MailboxOptions, PriorityEnvelope, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv,
};
use nexus_utils_embedded_rs::collections::queue::priority::ArcPriorityQueue;
use nexus_utils_embedded_rs::collections::queue::ring::ArcRingQueue;
use nexus_utils_embedded_rs::{
  Element, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, DEFAULT_CAPACITY, PRIORITY_LEVELS,
};

use crate::arc_mailbox::ArcSignal;

pub struct ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex, {
  control: ArcPriorityQueue<PriorityEnvelope<M>, RM>,
  regular: ArcRingQueue<PriorityEnvelope<M>, RM>,
  regular_capacity: usize,
}

impl<M, RM> Clone for ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      control: self.control.clone(),
      regular: self.regular.clone(),
      regular_capacity: self.regular_capacity,
    }
  }
}

impl<M, RM> ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  fn new(_levels: usize, control_per_level: usize, regular_capacity: usize) -> Self {
    let control = ArcPriorityQueue::new(control_per_level).with_dynamic(control_per_level == 0);
    let regular = if regular_capacity == 0 {
      ArcRingQueue::new(DEFAULT_CAPACITY).with_dynamic(true)
    } else {
      ArcRingQueue::new(regular_capacity).with_dynamic(false)
    };

    Self {
      control,
      regular,
      regular_capacity,
    }
  }

  fn offer(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    if envelope.is_control() {
      self.control.offer(envelope)
    } else {
      if self.regular_capacity > 0 {
        let len = self.regular.len().to_usize();
        if len >= self.regular_capacity {
          return Err(QueueError::Full(envelope));
        }
      }
      self.regular.offer(envelope)
    }
  }

  fn poll(&self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    if let Some(envelope) = self.control.poll()? {
      return Ok(Some(envelope));
    }
    self.regular.poll()
  }

  fn clean_up(&self) {
    self.control.clean_up();
    self.regular.clean_up();
  }

  fn len(&self) -> QueueSize {
    let control_len = self.control.len().to_usize();
    let regular_len = self.regular.len().to_usize();
    QueueSize::limited(control_len.saturating_add(regular_len))
  }

  fn capacity(&self) -> QueueSize {
    let control_cap = self.control.capacity();
    let regular_cap = if self.regular_capacity == 0 {
      QueueSize::limitless()
    } else {
      QueueSize::limited(self.regular_capacity)
    };

    if control_cap.is_limitless() || regular_cap.is_limitless() {
      QueueSize::limitless()
    } else {
      let total = control_cap.to_usize().saturating_add(regular_cap.to_usize());
      QueueSize::limited(total)
    }
  }
}

impl<M, RM> QueueBase<PriorityEnvelope<M>> for ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.len()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity()
  }
}

impl<M, RM> QueueWriter<PriorityEnvelope<M>> for ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  fn offer_mut(&mut self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.offer(envelope)
  }
}

impl<M, RM> QueueReader<PriorityEnvelope<M>> for ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  fn poll_mut(&mut self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    self.poll()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<M, RM> QueueRw<PriorityEnvelope<M>> for ArcPriorityQueues<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  fn offer(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.offer(envelope)
  }

  fn poll(&self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    self.poll()
  }

  fn clean_up(&self) {
    self.clean_up();
  }
}

#[derive(Clone)]
pub struct ArcPriorityMailbox<M, RM = CriticalSectionRawMutex>
where
  M: Element,
  RM: RawMutex, {
  inner: QueueMailbox<ArcPriorityQueues<M, RM>, ArcSignal<RM>>,
}

#[derive(Clone)]
pub struct ArcPriorityMailboxSender<M, RM = CriticalSectionRawMutex>
where
  M: Element,
  RM: RawMutex, {
  inner: QueueMailboxProducer<ArcPriorityQueues<M, RM>, ArcSignal<RM>>,
}

#[derive(Clone, Debug)]
pub struct ArcPriorityMailboxRuntime<RM = CriticalSectionRawMutex>
where
  RM: RawMutex, {
  control_capacity_per_level: usize,
  regular_capacity: usize,
  levels: usize,
  _marker: PhantomData<RM>,
}

impl<RM> Default for ArcPriorityMailboxRuntime<RM>
where
  RM: RawMutex,
{
  fn default() -> Self {
    Self {
      control_capacity_per_level: DEFAULT_CAPACITY,
      regular_capacity: DEFAULT_CAPACITY,
      levels: PRIORITY_LEVELS,
      _marker: PhantomData,
    }
  }
}

impl<RM> ArcPriorityMailboxRuntime<RM>
where
  RM: RawMutex,
{
  pub const fn new(control_capacity_per_level: usize) -> Self {
    Self {
      control_capacity_per_level,
      regular_capacity: DEFAULT_CAPACITY,
      levels: PRIORITY_LEVELS,
      _marker: PhantomData,
    }
  }

  pub fn with_levels(mut self, levels: usize) -> Self {
    self.levels = levels.max(1);
    self
  }

  pub fn with_regular_capacity(mut self, capacity: usize) -> Self {
    self.regular_capacity = capacity;
    self
  }

  pub fn mailbox<M>(&self, options: MailboxOptions) -> (ArcPriorityMailbox<M, RM>, ArcPriorityMailboxSender<M, RM>)
  where
    M: Element, {
    let control_per_level = self.resolve_control_capacity(options.priority_capacity);
    let regular_capacity = self.resolve_regular_capacity(options.capacity);
    let queue = ArcPriorityQueues::new(self.levels, control_per_level, regular_capacity);
    let signal = ArcSignal::default();
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = mailbox.producer();
    (
      ArcPriorityMailbox { inner: mailbox },
      ArcPriorityMailboxSender { inner: sender },
    )
  }

  fn resolve_control_capacity(&self, requested: QueueSize) -> usize {
    match requested {
      QueueSize::Limitless => self.control_capacity_per_level,
      QueueSize::Limited(value) => value,
    }
  }

  fn resolve_regular_capacity(&self, requested: QueueSize) -> usize {
    match requested {
      QueueSize::Limitless => self.regular_capacity,
      QueueSize::Limited(value) => value,
    }
  }
}

impl<M, RM> ArcPriorityMailbox<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  pub fn new(control_capacity_per_level: usize) -> (Self, ArcPriorityMailboxSender<M, RM>) {
    ArcPriorityMailboxRuntime::<RM>::new(control_capacity_per_level).mailbox(MailboxOptions::default())
  }

  pub fn inner(&self) -> &QueueMailbox<ArcPriorityQueues<M, RM>, ArcSignal<RM>> {
    &self.inner
  }
}

impl<M, RM> Mailbox<PriorityEnvelope<M>> for ArcPriorityMailbox<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  type RecvFuture<'a>
    = QueueMailboxRecv<'a, ArcPriorityQueues<M, RM>, ArcSignal<RM>, PriorityEnvelope<M>>
  where
    Self: 'a;
  type SendError = QueueError<PriorityEnvelope<M>>;

  fn try_send(&self, message: PriorityEnvelope<M>) -> Result<(), Self::SendError> {
    self.inner.try_send(message)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    self.inner.recv()
  }

  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }

  fn close(&self) {
    self.inner.close();
  }

  fn is_closed(&self) -> bool {
    self.inner.is_closed()
  }
}

impl<M, RM> ArcPriorityMailboxSender<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  pub fn try_send(&self, message: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.inner.try_send(message)
  }

  pub async fn send(&self, message: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.inner.send(message).await
  }

  pub fn try_send_with_priority(&self, message: M, priority: i8) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.try_send(PriorityEnvelope::new(message, priority))
  }

  pub async fn send_with_priority(&self, message: M, priority: i8) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.send(PriorityEnvelope::new(message, priority)).await
  }

  pub fn try_send_control_with_priority(
    &self,
    message: M,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.try_send(PriorityEnvelope::control(message, priority))
  }

  pub async fn send_control_with_priority(
    &self,
    message: M,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.send(PriorityEnvelope::control(message, priority)).await
  }

  pub fn inner(&self) -> &QueueMailboxProducer<ArcPriorityQueues<M, RM>, ArcSignal<RM>> {
    &self.inner
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::sync::atomic::{AtomicBool, Ordering};
  use critical_section::{Impl, RawRestoreState};
  use nexus_utils_embedded_rs::{QueueSize, DEFAULT_PRIORITY};

  fn prepare() {
    init_critical_section();
  }

  struct TestCriticalSection;

  static CS_LOCK: AtomicBool = AtomicBool::new(false);
  static CS_INIT: AtomicBool = AtomicBool::new(false);

  unsafe impl Impl for TestCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
      while CS_LOCK
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
      {}
      ()
    }

    unsafe fn release(_: RawRestoreState) {
      CS_LOCK.store(false, Ordering::SeqCst);
    }
  }

  fn init_critical_section() {
    if CS_INIT
      .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      critical_section::set_impl!(TestCriticalSection);
    }
  }

  #[test]
  fn priority_mailbox_orders_messages_by_priority() {
    prepare();
    let runtime = ArcPriorityMailboxRuntime::<CriticalSectionRawMutex>::default();
    let (mailbox, sender) = runtime.mailbox::<u8>(MailboxOptions::default());

    sender
      .try_send_with_priority(10, DEFAULT_PRIORITY)
      .expect("low priority");
    sender
      .try_send_control_with_priority(99, DEFAULT_PRIORITY + 7)
      .expect("high priority");
    sender
      .try_send_control_with_priority(20, DEFAULT_PRIORITY + 3)
      .expect("medium priority");

    let first = mailbox.inner().queue().poll().unwrap().unwrap();
    let second = mailbox.inner().queue().poll().unwrap().unwrap();
    let third = mailbox.inner().queue().poll().unwrap().unwrap();

    assert_eq!(first.into_parts(), (99, DEFAULT_PRIORITY + 7));
    assert_eq!(second.into_parts(), (20, DEFAULT_PRIORITY + 3));
    assert_eq!(third.into_parts(), (10, DEFAULT_PRIORITY));
  }

  #[test]
  fn priority_mailbox_capacity_split() {
    prepare();
    let runtime = ArcPriorityMailboxRuntime::<CriticalSectionRawMutex>::default();
    let options = MailboxOptions::with_capacities(QueueSize::limited(2), QueueSize::limited(2));
    let (mailbox, sender) = runtime.mailbox::<u8>(options);

    assert!(!mailbox.capacity().is_limitless());

    sender
      .try_send_control_with_priority(1, DEFAULT_PRIORITY + 3)
      .expect("control enqueue");
    sender
      .try_send_with_priority(2, DEFAULT_PRIORITY)
      .expect("regular enqueue");
    sender
      .try_send_with_priority(3, DEFAULT_PRIORITY)
      .expect("second regular enqueue");

    let err = sender
      .try_send_with_priority(4, DEFAULT_PRIORITY)
      .expect_err("regular capacity reached");
    assert!(matches!(err, QueueError::Full(_)));
  }

  #[test]
  fn control_queue_preempts_regular_messages() {
    prepare();
    let runtime = ArcPriorityMailboxRuntime::<CriticalSectionRawMutex>::default();
    let (mailbox, sender) = runtime.mailbox::<u32>(MailboxOptions::default());

    sender
      .try_send_with_priority(1, DEFAULT_PRIORITY)
      .expect("regular message");
    sender
      .try_send_control_with_priority(99, DEFAULT_PRIORITY + 5)
      .expect("control message");

    let first = mailbox.inner().queue().poll().unwrap().unwrap();
    let second = mailbox.inner().queue().poll().unwrap().unwrap();

    assert_eq!(first.into_parts(), (99, DEFAULT_PRIORITY + 5));
    assert_eq!(second.into_parts(), (1, DEFAULT_PRIORITY));
  }
}
