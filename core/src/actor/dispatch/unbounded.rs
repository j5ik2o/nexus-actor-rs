use crate::actor::dispatch::mailbox::sync_queue_handles::SyncMailboxQueue;
use crate::actor::dispatch::mailbox::DefaultMailbox;
use crate::actor::dispatch::mailbox::MailboxHandle;
use crate::actor::dispatch::mailbox_middleware::MailboxMiddlewareHandle;
use crate::actor::dispatch::mailbox_producer::MailboxProducer;
use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{
  MpscUnboundedChannelQueue, PriorityQueue, QueueError, QueueSize, RingQueue, SyncQueueBase, SyncQueueReader,
  SyncQueueSupport, SyncQueueWriter,
};

#[derive(Debug, Clone)]
pub(crate) struct UnboundedMailboxQueue<Q: SyncMailboxQueue> {
  user_mailbox: Q,
}

impl<Q: SyncMailboxQueue> UnboundedMailboxQueue<Q> {
  pub fn new(user_mailbox: Q) -> Self {
    UnboundedMailboxQueue { user_mailbox }
  }
}

impl<Q: SyncMailboxQueue> SyncQueueBase<MessageHandle> for UnboundedMailboxQueue<Q> {
  fn len(&self) -> QueueSize {
    self.user_mailbox.len()
  }

  fn capacity(&self) -> QueueSize {
    self.user_mailbox.capacity()
  }
}

impl<Q: SyncMailboxQueue> SyncQueueWriter<MessageHandle> for UnboundedMailboxQueue<Q> {
  fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.user_mailbox.offer(element)
  }
}

impl<Q: SyncMailboxQueue> SyncQueueReader<MessageHandle> for UnboundedMailboxQueue<Q> {
  fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.user_mailbox.poll()
  }

  fn clean_up(&mut self) {
    self.user_mailbox.clean_up();
  }
}

impl<Q: SyncMailboxQueue> SyncQueueSupport for UnboundedMailboxQueue<Q> {}

// ---

pub fn unbounded_mailbox_creator_with_opts(
  mailbox_stats: impl IntoIterator<Item = MailboxMiddlewareHandle> + Send + Sync,
) -> MailboxProducer {
  let cloned_mailbox_stats = mailbox_stats.into_iter().collect::<Vec<_>>();
  MailboxProducer::new(move || {
    let cloned_mailbox_stats = cloned_mailbox_stats.clone();
    async move {
      let user_queue = UnboundedMailboxQueue::new(RingQueue::new(10));
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      MailboxHandle::new(
        DefaultMailbox::new(user_queue, system_queue)
          .with_middlewares(cloned_mailbox_stats.clone())
          .await,
      )
    }
  })
}

pub fn unbounded_mailbox_creator() -> MailboxProducer {
  unbounded_mailbox_creator_with_opts([])
}

pub fn unbounded_priority_mailbox_creator_with_opts(
  mailbox_stats: impl IntoIterator<Item = MailboxMiddlewareHandle> + Send + Sync,
) -> MailboxProducer {
  let cloned_mailbox_stats = mailbox_stats.into_iter().collect::<Vec<_>>();
  MailboxProducer::new(move || {
    let cloned_mailbox_stats = cloned_mailbox_stats.clone();
    async move {
      let user_queue = UnboundedMailboxQueue::new(PriorityQueue::new(|| RingQueue::new(10)));
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      MailboxHandle::new(
        DefaultMailbox::new(user_queue, system_queue)
          .with_middlewares(cloned_mailbox_stats.clone())
          .await,
      )
    }
  })
}

pub fn unbounded_priority_mailbox_creator() -> MailboxProducer {
  unbounded_priority_mailbox_creator_with_opts([])
}

pub fn unbounded_mpsc_mailbox_creator_with_opts(
  mailbox_stats: impl IntoIterator<Item = MailboxMiddlewareHandle> + Send + Sync,
) -> MailboxProducer {
  let cloned_mailbox_stats = mailbox_stats.into_iter().collect::<Vec<_>>();
  MailboxProducer::new(move || {
    let cloned_mailbox_stats = cloned_mailbox_stats.clone();
    async move {
      let user_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
      MailboxHandle::new(
        DefaultMailbox::new(user_queue, system_queue)
          .with_middlewares(cloned_mailbox_stats.clone())
          .await,
      )
    }
  })
}

pub fn unbounded_mpsc_mailbox_creator() -> MailboxProducer {
  unbounded_mpsc_mailbox_creator_with_opts([])
}
