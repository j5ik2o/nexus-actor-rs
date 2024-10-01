use dashmap::DashMap;
use std::hash::Hash;

pub trait DashMapExtension<K: Eq + Hash, V: Clone> {
  fn load_or_store(&self, key: K, value: V) -> (V, bool);
}

impl<K: Eq + Hash, V: Clone> DashMapExtension<K, V> for DashMap<K, V> {
  fn load_or_store(&self, key: K, value: V) -> (V, bool) {
    match self.entry(key) {
      dashmap::mapref::entry::Entry::Occupied(entry) => (entry.get().clone(), true),
      dashmap::mapref::entry::Entry::Vacant(entry) => (entry.insert(value).clone(), false),
    }
  }
}
