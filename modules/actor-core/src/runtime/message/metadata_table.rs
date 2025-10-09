use alloc::vec::Vec;

use spin::{Mutex, Once};

use crate::api::MessageMetadata;

/// メタデータを参照するためのキー型。
pub type MetadataKey = u32;

struct MetadataTableInner {
  entries: Vec<Option<MessageMetadata>>,
  free_list: Vec<MetadataKey>,
}

impl MetadataTableInner {
  fn new() -> Self {
    Self {
      entries: Vec::new(),
      free_list: Vec::new(),
    }
  }
}

pub struct MetadataTable {
  inner: Mutex<MetadataTableInner>,
}

impl MetadataTable {
  pub fn new() -> Self {
    Self {
      inner: Mutex::new(MetadataTableInner::new()),
    }
  }

  pub fn store(&self, metadata: MessageMetadata) -> MetadataKey {
    let mut guard = self.inner.lock();
    if let Some(key) = guard.free_list.pop() {
      guard.entries[key as usize] = Some(metadata);
      key
    } else {
      let key = guard.entries.len() as MetadataKey;
      guard.entries.push(Some(metadata));
      key
    }
  }

  pub fn take(&self, key: MetadataKey) -> Option<MessageMetadata> {
    let mut guard = self.inner.lock();
    let index = key as usize;
    if index >= guard.entries.len() {
      return None;
    }
    let entry = guard.entries[index].take();
    if entry.is_some() {
      guard.free_list.push(key);
    }
    entry
  }
}

fn global_table() -> &'static MetadataTable {
  static TABLE: Once<MetadataTable> = Once::new();
  TABLE.call_once(MetadataTable::new)
}

pub fn store_metadata(metadata: MessageMetadata) -> MetadataKey {
  global_table().store(metadata)
}

pub fn take_metadata(key: MetadataKey) -> Option<MessageMetadata> {
  global_table().take(key)
}

#[cfg(test)]
pub(crate) fn outstanding_metadata_count() -> usize {
  let table = global_table();
  let guard = table.inner.lock();
  guard.entries.iter().filter(|entry| entry.is_some()).count()
}
