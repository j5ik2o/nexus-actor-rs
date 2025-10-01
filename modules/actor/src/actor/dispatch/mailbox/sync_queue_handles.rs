use std::fmt::Debug;
use std::sync::Arc;

use nexus_utils_core_rs::collections::{
  QueueError, QueueSize, SyncQueueBase, SyncQueueReader, SyncQueueSupport, SyncQueueWriter,
};
use parking_lot::Mutex;

use crate::actor::message::MessageHandle;

pub(crate) trait SyncMailboxQueue:
  SyncQueueWriter<MessageHandle>
  + SyncQueueReader<MessageHandle>
  + SyncQueueBase<MessageHandle>
  + SyncQueueSupport
  + Send
  + Sync
  + Clone
  + 'static {
}

impl<T> SyncMailboxQueue for T where
  T: SyncQueueWriter<MessageHandle>
    + SyncQueueReader<MessageHandle>
    + SyncQueueBase<MessageHandle>
    + SyncQueueSupport
    + Send
    + Sync
    + Clone
    + 'static
{
}

#[derive(Debug, Clone)]
pub(crate) struct SyncQueueWriterHandle<Q>
where
  Q: SyncMailboxQueue, {
  inner: Arc<Mutex<Q>>,
}

impl<Q> SyncQueueWriterHandle<Q>
where
  Q: SyncMailboxQueue,
{
  pub fn new(inner: Arc<Mutex<Q>>) -> Self {
    Self { inner }
  }

  pub fn offer_sync(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let mut guard = self.inner.lock();
    guard.offer(element)
  }

  pub async fn offer(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.offer_sync(element)
  }
}

#[derive(Debug, Clone)]
pub(crate) struct SyncQueueReaderHandle<Q>
where
  Q: SyncMailboxQueue, {
  inner: Arc<Mutex<Q>>,
}

impl<Q> SyncQueueReaderHandle<Q>
where
  Q: SyncMailboxQueue,
{
  pub fn new(inner: Arc<Mutex<Q>>) -> Self {
    Self { inner }
  }

  pub fn poll_sync(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let mut guard = self.inner.lock();
    guard.poll()
  }

  pub fn clean_up_sync(&self) {
    let mut guard = self.inner.lock();
    guard.clean_up();
  }

  pub fn len_sync(&self) -> QueueSize {
    let guard = self.inner.lock();
    guard.len()
  }

  pub async fn poll(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.poll_sync()
  }

  pub async fn clean_up(&self) {
    self.clean_up_sync();
  }

  pub async fn len(&self) -> QueueSize {
    self.len_sync()
  }
}

#[derive(Debug, Clone)]
pub(crate) struct SyncMailboxQueueHandles<Q>
where
  Q: SyncMailboxQueue, {
  shared: Arc<Mutex<Q>>,
}

impl<Q> SyncMailboxQueueHandles<Q>
where
  Q: SyncMailboxQueue,
{
  pub fn new(queue: Q) -> Self {
    Self {
      shared: Arc::new(Mutex::new(queue)),
    }
  }

  pub fn writer_handle(&self) -> SyncQueueWriterHandle<Q> {
    SyncQueueWriterHandle::new(self.shared.clone())
  }

  pub fn reader_handle(&self) -> SyncQueueReaderHandle<Q> {
    SyncQueueReaderHandle::new(self.shared.clone())
  }
}
