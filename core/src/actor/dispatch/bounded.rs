#[cfg(test)]
mod tests;

use crate::actor::dispatch::mailbox::DefaultMailbox;
use crate::actor::dispatch::mailbox::MailboxHandle;
use crate::actor::dispatch::mailbox_middleware::MailboxMiddlewareHandle;
use crate::actor::dispatch::mailbox_producer::MailboxProducer;
use crate::actor::dispatch::unbounded::UnboundedMailboxQueue;
use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{
  MpscUnboundedChannelQueue, QueueError, QueueSize, RingQueue, SyncQueueBase, SyncQueueReader, SyncQueueSupport,
  SyncQueueWriter,
};
use std::fmt::Debug;

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

impl SyncQueueBase<MessageHandle> for BoundedMailboxQueue {
  fn len(&self) -> QueueSize {
    self.user_mailbox.len()
  }

  fn capacity(&self) -> QueueSize {
    self.user_mailbox.capacity()
  }
}

impl SyncQueueWriter<MessageHandle> for BoundedMailboxQueue {
  fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let len = self.user_mailbox.len();
    if self.dropping && len == QueueSize::Limited(self.initial_capacity) {
      let _ = self.user_mailbox.poll()?;
    }
    self.user_mailbox.offer(element)
  }
}

impl SyncQueueReader<MessageHandle> for BoundedMailboxQueue {
  fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.user_mailbox.poll()
  }

  fn clean_up(&mut self) {
    self.user_mailbox.clean_up();
  }
}

impl SyncQueueSupport for BoundedMailboxQueue {}

pub fn bounded_mailbox_creator_with_opts(
  size: usize,
  dropping: bool,
  mailbox_stats: impl IntoIterator<Item = MailboxMiddlewareHandle> + Send + Sync,
) -> MailboxProducer {
  let cloned_mailbox_stats = mailbox_stats.into_iter().collect::<Vec<_>>();
  MailboxProducer::new(move || {
    let cloned_mailbox_stats = cloned_mailbox_stats.clone();
    async move {
      let user_queue = BoundedMailboxQueue::new(RingQueue::new(size), size, dropping);
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      let mailbox = DefaultMailbox::new(user_queue, system_queue)
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
