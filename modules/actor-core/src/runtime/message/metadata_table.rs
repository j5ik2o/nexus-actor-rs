use alloc::vec::Vec;

use crate::api::MessageMetadata;
use crate::runtime::mailbox::traits::{MailboxConcurrency, SingleThread, ThreadSafe};

#[cfg(not(target_has_atomic = "ptr"))]
use core::cell::RefCell;
#[cfg(not(target_has_atomic = "ptr"))]
use critical_section::Mutex;
#[cfg(target_has_atomic = "ptr")]
use spin::{Mutex, Once};

/// Key type for referencing metadata.
pub type MetadataKey = u32;

/// Stored representation of message metadata keyed by concurrency mode.
#[derive(Debug, Clone)]
pub enum StoredMessageMetadata {
  ThreadSafe(MessageMetadata<ThreadSafe>),
  SingleThread(MessageMetadata<SingleThread>),
}

#[cfg(not(target_has_atomic = "ptr"))]
unsafe impl Send for StoredMessageMetadata {}

#[cfg(not(target_has_atomic = "ptr"))]
unsafe impl Sync for StoredMessageMetadata {}

/// Trait describing how metadata for a particular concurrency marker is stored.
pub trait MetadataStorageMode: MailboxConcurrency {
  /// Converts metadata into the stored representation.
  fn into_stored(metadata: MessageMetadata<Self>) -> StoredMessageMetadata;

  /// Attempts to extract metadata of this concurrency mode from the stored representation.
  fn from_stored(stored: StoredMessageMetadata) -> Option<MessageMetadata<Self>>;
}

impl MetadataStorageMode for ThreadSafe {
  fn into_stored(metadata: MessageMetadata<Self>) -> StoredMessageMetadata {
    StoredMessageMetadata::ThreadSafe(metadata)
  }

  fn from_stored(stored: StoredMessageMetadata) -> Option<MessageMetadata<Self>> {
    match stored {
      StoredMessageMetadata::ThreadSafe(metadata) => Some(metadata),
      StoredMessageMetadata::SingleThread(_) => None,
    }
  }
}

impl MetadataStorageMode for SingleThread {
  fn into_stored(metadata: MessageMetadata<Self>) -> StoredMessageMetadata {
    StoredMessageMetadata::SingleThread(metadata)
  }

  fn from_stored(stored: StoredMessageMetadata) -> Option<MessageMetadata<Self>> {
    match stored {
      StoredMessageMetadata::SingleThread(metadata) => Some(metadata),
      StoredMessageMetadata::ThreadSafe(_) => None,
    }
  }
}

struct MetadataTableInner {
  entries: Vec<Option<StoredMessageMetadata>>,
  free_list: Vec<MetadataKey>,
}

#[cfg(not(target_has_atomic = "ptr"))]
unsafe impl Send for MetadataTableInner {}

#[cfg(not(target_has_atomic = "ptr"))]
unsafe impl Sync for MetadataTableInner {}

impl MetadataTableInner {
  const fn new() -> Self {
    Self {
      entries: Vec::new(),
      free_list: Vec::new(),
    }
  }

  fn store<C>(&mut self, metadata: MessageMetadata<C>) -> MetadataKey
  where
    C: MetadataStorageMode, {
    let stored = C::into_stored(metadata);
    if let Some(key) = self.free_list.pop() {
      self.entries[key as usize] = Some(stored);
      key
    } else {
      let key = self.entries.len() as MetadataKey;
      self.entries.push(Some(stored));
      key
    }
  }

  fn discard(&mut self, key: MetadataKey) -> Option<StoredMessageMetadata> {
    let index = key as usize;
    if index >= self.entries.len() {
      return None;
    }
    let entry = self.entries[index].take();
    if entry.is_some() {
      self.free_list.push(key);
    }
    entry
  }

  fn take<C>(&mut self, key: MetadataKey) -> Option<MessageMetadata<C>>
  where
    C: MetadataStorageMode, {
    self.discard(key).and_then(C::from_stored)
  }
}

#[cfg(target_has_atomic = "ptr")]
pub struct MetadataTable {
  inner: Mutex<MetadataTableInner>,
}

#[cfg(target_has_atomic = "ptr")]
impl MetadataTable {
  const fn new() -> Self {
    Self {
      inner: Mutex::new(MetadataTableInner::new()),
    }
  }

  pub fn store<C>(&self, metadata: MessageMetadata<C>) -> MetadataKey
  where
    C: MetadataStorageMode, {
    let mut guard = self.inner.lock();
    guard.store(metadata)
  }

  pub fn take<C>(&self, key: MetadataKey) -> Option<MessageMetadata<C>>
  where
    C: MetadataStorageMode, {
    let mut guard = self.inner.lock();
    guard.take(key)
  }

  pub fn discard(&self, key: MetadataKey) {
    let mut guard = self.inner.lock();
    guard.discard(key);
  }
}

#[cfg(target_has_atomic = "ptr")]
fn global_table() -> &'static MetadataTable {
  static TABLE: Once<MetadataTable> = Once::new();
  TABLE.call_once(MetadataTable::new)
}

#[cfg(not(target_has_atomic = "ptr"))]
pub struct MetadataTable {
  inner: Mutex<RefCell<MetadataTableInner>>,
}

#[cfg(not(target_has_atomic = "ptr"))]
impl MetadataTable {
  const fn new() -> Self {
    Self {
      inner: Mutex::new(RefCell::new(MetadataTableInner::new())),
    }
  }

  fn with_inner<R>(&self, f: impl FnOnce(&mut MetadataTableInner) -> R) -> R {
    critical_section::with(|cs| {
      let mut guard = self.inner.borrow(cs).borrow_mut();
      f(&mut guard)
    })
  }

  pub fn store<C>(&self, metadata: MessageMetadata<C>) -> MetadataKey
  where
    C: MetadataStorageMode, {
    self.with_inner(|inner| inner.store(metadata))
  }

  pub fn take<C>(&self, key: MetadataKey) -> Option<MessageMetadata<C>>
  where
    C: MetadataStorageMode, {
    self.with_inner(|inner| inner.take(key))
  }

  pub fn discard(&self, key: MetadataKey) {
    self.with_inner(|inner| {
      inner.discard(key);
    });
  }
}

#[cfg(not(target_has_atomic = "ptr"))]
fn global_table() -> &'static MetadataTable {
  static TABLE: MetadataTable = MetadataTable::new();
  &TABLE
}

/// Stores a value in the global metadata table and returns its key.
pub fn store_metadata<C>(metadata: MessageMetadata<C>) -> MetadataKey
where
  C: MetadataStorageMode, {
  global_table().store(metadata)
}

/// Retrieves previously registered metadata and removes it from the table.
pub fn take_metadata<C>(key: MetadataKey) -> Option<MessageMetadata<C>>
where
  C: MetadataStorageMode, {
  global_table().take(key)
}

/// Drops metadata associated with the specified key without attempting to downcast it.
pub fn discard_metadata(key: MetadataKey) {
  global_table().discard(key);
}
