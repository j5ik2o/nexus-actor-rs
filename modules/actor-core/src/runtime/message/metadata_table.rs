use alloc::vec::Vec;

use crate::api::MessageMetadata;

#[cfg(not(target_has_atomic = "ptr"))]
use core::cell::RefCell;
#[cfg(not(target_has_atomic = "ptr"))]
use critical_section::Mutex;
#[cfg(target_has_atomic = "ptr")]
use spin::{Mutex, Once};

/// Key type for referencing metadata.
pub type MetadataKey = u32;

struct MetadataTableInner {
  entries: Vec<Option<MessageMetadata>>,
  free_list: Vec<MetadataKey>,
}

impl MetadataTableInner {
  const fn new() -> Self {
    Self {
      entries: Vec::new(),
      free_list: Vec::new(),
    }
  }

  fn store(&mut self, metadata: MessageMetadata) -> MetadataKey {
    if let Some(key) = self.free_list.pop() {
      self.entries[key as usize] = Some(metadata);
      key
    } else {
      let key = self.entries.len() as MetadataKey;
      self.entries.push(Some(metadata));
      key
    }
  }

  fn take(&mut self, key: MetadataKey) -> Option<MessageMetadata> {
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

  pub fn store(&self, metadata: MessageMetadata) -> MetadataKey {
    let mut guard = self.inner.lock();
    guard.store(metadata)
  }

  pub fn take(&self, key: MetadataKey) -> Option<MessageMetadata> {
    let mut guard = self.inner.lock();
    guard.take(key)
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

  pub fn store(&self, metadata: MessageMetadata) -> MetadataKey {
    self.with_inner(|inner| inner.store(metadata))
  }

  pub fn take(&self, key: MetadataKey) -> Option<MessageMetadata> {
    self.with_inner(|inner| inner.take(key))
  }
}

#[cfg(not(target_has_atomic = "ptr"))]
fn global_table() -> &'static MetadataTable {
  static TABLE: MetadataTable = MetadataTable::new();
  &TABLE
}

/// Stores a value in the global metadata table and returns its key.
pub fn store_metadata(metadata: MessageMetadata) -> MetadataKey {
  global_table().store(metadata)
}

/// Retrieves previously registered metadata and removes it from the table.
pub fn take_metadata(key: MetadataKey) -> Option<MessageMetadata> {
  global_table().take(key)
}
