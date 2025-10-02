use std::fmt::Debug;

use crate::actor::message::MessageHandle;
use nexus_actor_core_rs::actor::core_types::mailbox::CoreMailboxQueue;
use nexus_utils_std_rs::collections::RingQueue;
use nexus_utils_std_rs::collections::{
  MpscBoundedChannelQueue, MpscUnboundedChannelQueue, PriorityQueue, QueueError, QueueReader, QueueSupport, QueueWriter,
};

#[derive(Debug, Clone)]
pub(crate) struct UnboundedMpscCoreMailboxQueue {
  queue: MpscUnboundedChannelQueue<MessageHandle>,
}

impl UnboundedMpscCoreMailboxQueue {
  pub fn new(queue: MpscUnboundedChannelQueue<MessageHandle>) -> Self {
    Self { queue }
  }
}

impl CoreMailboxQueue for UnboundedMpscCoreMailboxQueue {
  type Error = QueueError<MessageHandle>;

  fn offer(&self, message: MessageHandle) -> Result<(), Self::Error> {
    self.queue.offer_shared(message)
  }

  fn poll(&self) -> Result<Option<MessageHandle>, Self::Error> {
    self.queue.poll_shared()
  }

  fn len(&self) -> usize {
    self.queue.len_shared()
  }

  fn clean_up(&self) {
    self.queue.clean_up_shared();
  }
}

#[derive(Debug, Clone)]
pub(crate) struct BoundedMpscCoreMailboxQueue {
  queue: MpscBoundedChannelQueue<MessageHandle>,
}

impl BoundedMpscCoreMailboxQueue {
  pub fn new(queue: MpscBoundedChannelQueue<MessageHandle>) -> Self {
    Self { queue }
  }
}

impl CoreMailboxQueue for BoundedMpscCoreMailboxQueue {
  type Error = QueueError<MessageHandle>;

  fn offer(&self, message: MessageHandle) -> Result<(), Self::Error> {
    self.queue.offer_shared(message)
  }

  fn poll(&self) -> Result<Option<MessageHandle>, Self::Error> {
    self.queue.poll_shared()
  }

  fn len(&self) -> usize {
    self.queue.len_shared()
  }

  fn clean_up(&self) {
    self.queue.clean_up_shared();
  }
}

#[derive(Debug, Clone)]
pub(crate) struct RingCoreMailboxQueue {
  queue: RingQueue<MessageHandle>,
}

impl RingCoreMailboxQueue {
  pub fn new(queue: RingQueue<MessageHandle>) -> Self {
    Self { queue }
  }
}

impl CoreMailboxQueue for RingCoreMailboxQueue {
  type Error = QueueError<MessageHandle>;

  fn offer(&self, message: MessageHandle) -> Result<(), Self::Error> {
    self.queue.offer_shared(message)
  }

  fn poll(&self) -> Result<Option<MessageHandle>, Self::Error> {
    self.queue.poll_shared()
  }

  fn len(&self) -> usize {
    self.queue.len_shared()
  }

  fn clean_up(&self) {
    self.queue.clean_up_shared();
  }
}

#[derive(Debug, Clone)]
pub(crate) struct PriorityCoreMailboxQueue<Q>
where
  Q: Clone + QueueReader<MessageHandle> + QueueWriter<MessageHandle> + QueueSupport + Send + Sync + 'static, {
  queue: PriorityQueue<MessageHandle, Q>,
}

impl<Q> PriorityCoreMailboxQueue<Q>
where
  Q: Clone + QueueReader<MessageHandle> + QueueWriter<MessageHandle> + QueueSupport + Send + Sync + 'static,
{
  pub fn new(queue: PriorityQueue<MessageHandle, Q>) -> Self {
    Self { queue }
  }
}

impl<Q> CoreMailboxQueue for PriorityCoreMailboxQueue<Q>
where
  Q: Clone + QueueReader<MessageHandle> + QueueWriter<MessageHandle> + QueueSupport + Send + Sync + 'static,
{
  type Error = QueueError<MessageHandle>;

  fn offer(&self, message: MessageHandle) -> Result<(), Self::Error> {
    self.queue.offer_shared(message)
  }

  fn poll(&self) -> Result<Option<MessageHandle>, Self::Error> {
    self.queue.poll_shared()
  }

  fn len(&self) -> usize {
    self.queue.len_shared()
  }

  fn clean_up(&self) {
    self.queue.clean_up_shared();
  }
}
