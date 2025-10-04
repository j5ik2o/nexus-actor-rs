use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use nexus_actor_core_rs::actor::core_types::mailbox::CoreMailboxQueue;
use nexus_utils_std_rs::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueSupport, QueueWriter};
use parking_lot::Mutex;

use crate::actor::message::MessageHandle;

pub(crate) trait SyncMailboxQueue:
  QueueWriter<MessageHandle>
  + QueueReader<MessageHandle>
  + QueueBase<MessageHandle>
  + QueueSupport
  + Send
  + Sync
  + Clone
  + 'static {
}

impl<T> SyncMailboxQueue for T where
  T: QueueWriter<MessageHandle>
    + QueueReader<MessageHandle>
    + QueueBase<MessageHandle>
    + QueueSupport
    + Send
    + Sync
    + Clone
    + 'static
{
}

#[derive(Debug, Clone)]
pub(crate) struct QueueWriterHandle<Q>
where
  Q: SyncMailboxQueue, {
  inner: Arc<Mutex<Q>>,
}

impl<Q> QueueWriterHandle<Q>
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
pub(crate) struct QueueReaderHandle<Q>
where
  Q: SyncMailboxQueue, {
  inner: Arc<Mutex<Q>>,
}

impl<Q> QueueReaderHandle<Q>
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

#[derive(Clone)]
pub(crate) struct SyncMailboxQueueHandles<Q>
where
  Q: SyncMailboxQueue, {
  shared: Arc<Mutex<Q>>,
  core_queue: Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
}

impl<Q> Debug for SyncMailboxQueueHandles<Q>
where
  Q: SyncMailboxQueue,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("SyncMailboxQueueHandles").finish_non_exhaustive()
  }
}

impl<Q> SyncMailboxQueueHandles<Q>
where
  Q: SyncMailboxQueue,
{
  pub fn new(queue: Q) -> Self {
    Self::new_with_core(queue, None)
  }

  pub fn new_with_core(
    queue: Q,
    core_queue: Option<Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>>,
  ) -> Self {
    let shared = Arc::new(Mutex::new(queue));
    let core_queue = core_queue.unwrap_or_else(|| {
      let adapter: Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync> =
        Arc::new(CoreMailboxQueueHandle::new(shared.clone()));
      adapter
    });
    Self { shared, core_queue }
  }

  pub fn writer_handle(&self) -> QueueWriterHandle<Q> {
    QueueWriterHandle::new(self.shared.clone())
  }

  pub fn reader_handle(&self) -> QueueReaderHandle<Q> {
    QueueReaderHandle::new(self.shared.clone())
  }

  pub fn core_queue_handle(&self) -> CoreMailboxQueueHandle<Q> {
    CoreMailboxQueueHandle::new(self.shared.clone())
  }

  pub fn core_queue(&self) -> Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync> {
    self.core_queue.clone()
  }
}

#[derive(Debug, Clone)]
pub(crate) struct CoreMailboxQueueHandle<Q>
where
  Q: SyncMailboxQueue, {
  shared: Arc<Mutex<Q>>,
}

impl<Q> CoreMailboxQueueHandle<Q>
where
  Q: SyncMailboxQueue,
{
  fn new(shared: Arc<Mutex<Q>>) -> Self {
    Self { shared }
  }

  fn with_lock<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut Q) -> R, {
    let mut guard = self.shared.lock();
    f(&mut guard)
  }
}

impl<Q> CoreMailboxQueue for CoreMailboxQueueHandle<Q>
where
  Q: SyncMailboxQueue,
{
  type Error = QueueError<MessageHandle>;

  fn offer(&self, message: MessageHandle) -> Result<(), Self::Error> {
    self.with_lock(|queue| queue.offer(message))
  }

  fn poll(&self) -> Result<Option<MessageHandle>, Self::Error> {
    self.with_lock(|queue| queue.poll())
  }

  fn len(&self) -> usize {
    self.with_lock(|queue| queue.len()).to_usize()
  }

  fn clean_up(&self) {
    self.with_lock(|queue| {
      queue.clean_up();
    });
  }
}
