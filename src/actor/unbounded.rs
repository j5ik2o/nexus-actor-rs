use async_trait::async_trait;

use crate::actor::dispatcher::MailboxMiddlewareHandle;
use crate::actor::mailbox::{DefaultMailbox, MailboxHandle, MailboxProduceFunc};
use crate::actor::message::MessageHandle;
use crate::util::queue::mpsc_unbounded_channel_queue::MpscUnboundedChannelQueue;
use crate::util::queue::priority_queue::PriorityQueue;
use crate::util::queue::ring_queue::RingQueue;
use crate::util::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug, Clone)]
pub struct UnboundedMailboxQueue<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> {
  user_mailbox: Q,
}

impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> UnboundedMailboxQueue<Q> {
  pub fn new(user_mailbox: Q) -> Self {
    UnboundedMailboxQueue { user_mailbox }
  }
}

#[async_trait]
impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueBase<MessageHandle> for UnboundedMailboxQueue<Q> {
  async fn len(&self) -> QueueSize {
    self.user_mailbox.len().await
  }

  async fn capacity(&self) -> QueueSize {
    self.user_mailbox.capacity().await
  }
}

#[async_trait]
impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueReader<MessageHandle>
  for UnboundedMailboxQueue<Q>
{
  async fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.user_mailbox.poll().await
  }

  async fn clean_up(&mut self) {
    self.user_mailbox.clean_up().await
  }
}

#[async_trait]
impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueWriter<MessageHandle>
  for UnboundedMailboxQueue<Q>
{
  async fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.user_mailbox.offer(element).await
  }
}

pub fn unbounded_mailbox_creator(mailbox_stats: Vec<MailboxMiddlewareHandle>) -> MailboxProduceFunc {
  MailboxProduceFunc::new(move || {
    let cloned_mailbox_stats = mailbox_stats.clone();
    Box::pin(async move {
      let user_queue = UnboundedMailboxQueue::new(RingQueue::new(10));
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      MailboxHandle::new(DefaultMailbox::new(user_queue, system_queue).with_middlewares(cloned_mailbox_stats.clone()))
    })
  })
}

pub fn unbounded_priority_mailbox_creator(mailbox_stats: Vec<MailboxMiddlewareHandle>) -> MailboxProduceFunc {
  MailboxProduceFunc::new(move || {
    let cloned_mailbox_stats = mailbox_stats.clone();
    Box::pin(async move {
      let user_queue = UnboundedMailboxQueue::new(PriorityQueue::new(|| RingQueue::new(10)));
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      MailboxHandle::new(DefaultMailbox::new(user_queue, system_queue).with_middlewares(cloned_mailbox_stats.clone()))
    })
  })
}

pub fn unbounded_mpsc_mailbox_creator(mailbox_stats: Vec<MailboxMiddlewareHandle>) -> MailboxProduceFunc {
  MailboxProduceFunc::new(move || {
    let cloned_mailbox_stats = mailbox_stats.clone();
    Box::pin(async move {
      let user_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      MailboxHandle::new(DefaultMailbox::new(user_queue, system_queue).with_middlewares(cloned_mailbox_stats.clone()))
    })
  })
}
