#[cfg(test)]
mod tests;

use crate::actor::dispatch::mailbox::DefaultMailbox;
use crate::actor::dispatch::mailbox::MailboxHandle;
use crate::actor::dispatch::mailbox_middleware::MailboxMiddlewareHandle;
use crate::actor::dispatch::mailbox_producer::MailboxProducer;
use crate::actor::dispatch::unbounded::UnboundedMailboxQueue;
use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use nexus_actor_utils_rs::collections::{
  MpscUnboundedChannelQueue, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingQueue,
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

#[async_trait]
impl QueueBase<MessageHandle> for BoundedMailboxQueue {
  async fn len(&self) -> QueueSize {
    self.user_mailbox.len().await
  }

  async fn capacity(&self) -> QueueSize {
    self.user_mailbox.capacity().await
  }
}

#[async_trait]
impl QueueWriter<MessageHandle> for BoundedMailboxQueue {
  async fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let len = self.user_mailbox.len().await;
    if self.dropping && len == QueueSize::Limited(self.initial_capacity) {
      let _ = self.user_mailbox.poll().await;
    }
    self.user_mailbox.offer(element).await
  }
}

#[async_trait]
impl QueueReader<MessageHandle> for BoundedMailboxQueue {
  async fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.user_mailbox.poll().await
  }

  async fn clean_up(&mut self) {
    self.user_mailbox.clean_up().await
  }
}

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
      MailboxHandle::new(
        DefaultMailbox::new(user_queue, system_queue)
          .with_middlewares(cloned_mailbox_stats.clone())
          .await,
      )
    }
  })
}

pub fn bounded_mailbox_creator(size: usize, dropping: bool) -> MailboxProducer {
  bounded_mailbox_creator_with_opts(size, dropping, [])
}
