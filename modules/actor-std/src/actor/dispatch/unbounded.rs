use crate::actor::dispatch::mailbox::core_queue_adapters::{
  PriorityCoreMailboxQueue, RingCoreMailboxQueue, UnboundedMpscCoreMailboxQueue,
};
use crate::actor::dispatch::mailbox::sync_queue_handles::{SyncMailboxQueue, SyncMailboxQueueHandles};
use crate::actor::dispatch::mailbox::DefaultMailbox;
use crate::actor::dispatch::mailbox::MailboxHandle;
use crate::actor::dispatch::mailbox_middleware::MailboxMiddlewareHandle;
use crate::actor::dispatch::mailbox_producer::MailboxProducer;
use crate::actor::message::MessageHandle;
use nexus_utils_std_rs::collections::{
  MpscUnboundedChannelQueue, PriorityQueue, QueueBase, QueueError, QueueReader, QueueSize, QueueSupport, QueueWriter,
  RingQueue,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct UnboundedMailboxQueue<Q: SyncMailboxQueue> {
  user_mailbox: Q,
}

impl<Q: SyncMailboxQueue> UnboundedMailboxQueue<Q> {
  pub fn new(user_mailbox: Q) -> Self {
    UnboundedMailboxQueue { user_mailbox }
  }
}

impl<Q: SyncMailboxQueue> QueueBase<MessageHandle> for UnboundedMailboxQueue<Q> {
  fn len(&self) -> QueueSize {
    self.user_mailbox.len()
  }

  fn capacity(&self) -> QueueSize {
    self.user_mailbox.capacity()
  }
}

impl<Q: SyncMailboxQueue> QueueWriter<MessageHandle> for UnboundedMailboxQueue<Q> {
  fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.user_mailbox.offer(element)
  }
}

impl<Q: SyncMailboxQueue> QueueReader<MessageHandle> for UnboundedMailboxQueue<Q> {
  fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.user_mailbox.poll()
  }

  fn clean_up(&mut self) {
    self.user_mailbox.clean_up();
  }
}

impl<Q: SyncMailboxQueue> QueueSupport for UnboundedMailboxQueue<Q> {}

// ---

pub fn unbounded_mailbox_creator_with_opts(
  mailbox_stats: impl IntoIterator<Item = MailboxMiddlewareHandle> + Send + Sync,
) -> MailboxProducer {
  let cloned_mailbox_stats = mailbox_stats.into_iter().collect::<Vec<_>>();
  MailboxProducer::new(move || {
    let cloned_mailbox_stats = cloned_mailbox_stats.clone();
    async move {
      let ring_queue = RingQueue::new(10);
      let user_core = Arc::new(RingCoreMailboxQueue::new(ring_queue.clone()));
      let user_handles =
        SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(ring_queue), Some(user_core));

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
      let priority_queue = PriorityQueue::new(|| RingQueue::new(10));
      let user_core = Arc::new(PriorityCoreMailboxQueue::new(priority_queue.clone()));
      let user_handles =
        SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(priority_queue), Some(user_core));

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
      let user_mpsc = MpscUnboundedChannelQueue::new();
      let user_core = Arc::new(UnboundedMpscCoreMailboxQueue::new(user_mpsc.clone()));
      let user_queue = UnboundedMailboxQueue::new(user_mpsc);
      let user_handles = SyncMailboxQueueHandles::new_with_core(user_queue, Some(user_core));

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

pub fn unbounded_mpsc_mailbox_creator() -> MailboxProducer {
  unbounded_mpsc_mailbox_creator_with_opts([])
}
