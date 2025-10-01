#[cfg(test)]
mod tests;

use crate::actor::dispatch::mailbox::core_queue_adapters::{RingCoreMailboxQueue, UnboundedMpscCoreMailboxQueue};
use crate::actor::dispatch::mailbox::sync_queue_handles::SyncMailboxQueueHandles;
use crate::actor::dispatch::mailbox::DefaultMailbox;
use crate::actor::dispatch::mailbox::MailboxHandle;
use crate::actor::dispatch::mailbox_middleware::MailboxMiddlewareHandle;
use crate::actor::dispatch::mailbox_producer::MailboxProducer;
use crate::actor::dispatch::unbounded::UnboundedMailboxQueue;
use crate::actor::message::MessageHandle;
use nexus_utils_std_rs::collections::{
  MpscUnboundedChannelQueue, QueueBase, QueueError, QueueReader, QueueSize, QueueSupport, QueueWriter, RingQueue,
};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BoundedMailboxQueue {
  user_mailbox: RingQueue<MessageHandle>,
  initial_capacity: usize,
  dropping: bool,
}

impl BoundedMailboxQueue {
  pub(crate) fn new(user_mailbox: RingQueue<MessageHandle>, initial_capacity: usize, dropping: bool) -> Self {
    BoundedMailboxQueue {
      user_mailbox,
      initial_capacity,
      dropping,
    }
  }
}

impl QueueBase<MessageHandle> for BoundedMailboxQueue {
  fn len(&self) -> QueueSize {
    self.user_mailbox.len()
  }

  fn capacity(&self) -> QueueSize {
    self.user_mailbox.capacity()
  }
}

impl QueueWriter<MessageHandle> for BoundedMailboxQueue {
  fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let len = self.user_mailbox.len();
    if self.dropping && len == QueueSize::Limited(self.initial_capacity) {
      let _ = self.user_mailbox.poll()?;
    }
    self.user_mailbox.offer(element)
  }
}

impl QueueReader<MessageHandle> for BoundedMailboxQueue {
  fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.user_mailbox.poll()
  }

  fn clean_up(&mut self) {
    self.user_mailbox.clean_up();
  }
}

impl QueueSupport for BoundedMailboxQueue {}

pub fn bounded_mailbox_creator_with_opts(
  size: usize,
  dropping: bool,
  mailbox_stats: impl IntoIterator<Item = MailboxMiddlewareHandle> + Send + Sync,
) -> MailboxProducer {
  let cloned_mailbox_stats = mailbox_stats.into_iter().collect::<Vec<_>>();
  MailboxProducer::new(move || {
    let cloned_mailbox_stats = cloned_mailbox_stats.clone();
    async move {
      let ring_queue = RingQueue::new(size);
      let user_core = Arc::new(RingCoreMailboxQueue::new(ring_queue.clone()));
      let user_handles =
        SyncMailboxQueueHandles::new_with_core(BoundedMailboxQueue::new(ring_queue, size, dropping), Some(user_core));

      let system_mpsc = MpscUnboundedChannelQueue::new();
      let system_core = Arc::new(UnboundedMpscCoreMailboxQueue::new(system_mpsc.clone()));
      let system_queue = UnboundedMailboxQueue::new(system_mpsc);
      let system_handles = SyncMailboxQueueHandles::new_with_core(system_queue, Some(system_core));

      let mailbox = DefaultMailbox::from_handles(user_handles, system_handles)
        .with_middlewares(cloned_mailbox_stats.clone())
        .await;
      let sync = mailbox.to_sync_handle();
      MailboxHandle::new_with_sync(mailbox, Some(sync))
    }
  })
}

pub fn bounded_mailbox_creator(size: usize, dropping: bool) -> MailboxProducer {
  bounded_mailbox_creator_with_opts(size, dropping, [])
}
