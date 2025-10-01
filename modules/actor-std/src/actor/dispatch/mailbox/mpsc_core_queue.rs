use std::fmt::Debug;

use crate::actor::message::MessageHandle;
use nexus_actor_core_rs::actor::core_types::mailbox::CoreMailboxQueue;
use nexus_utils_std_rs::collections::{MpscBoundedChannelQueue, MpscUnboundedChannelQueue, QueueError};

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
