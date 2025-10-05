use std::ops::Deref;
use std::sync::{Arc, Mutex};

use nexus_utils_core_rs::sync::Shared;
use nexus_utils_core_rs::{MpscBuffer, QueueStorage, SharedMpscHandle, SharedQueueHandle};

#[derive(Debug)]
pub struct ArcShared<T>(Arc<T>);

impl<T> ArcShared<T> {
  pub fn new(value: T) -> Self {
    Self(Arc::new(value))
  }

  pub fn from_arc(inner: Arc<T>) -> Self {
    Self(inner)
  }

  pub fn into_arc(self) -> Arc<T> {
    self.0
  }
}

impl<T> Deref for ArcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> Shared<T> for ArcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Arc::try_unwrap(self.0).map_err(ArcShared)
  }
}

impl<T> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T, E> SharedQueueHandle<E> for ArcShared<T>
where
  T: QueueStorage<E>,
{
  type Storage = T;

  fn storage(&self) -> &Self::Storage {
    &self.0
  }
}

impl<T> SharedMpscHandle<T> for ArcShared<Mutex<MpscBuffer<T>>> {
  type Storage = Mutex<MpscBuffer<T>>;

  fn storage(&self) -> &Self::Storage {
    &self.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_utils_core_rs::{QueueBase, RingBuffer};
  use std::sync::Mutex;

  #[test]
  fn arc_shared_try_unwrap_behavior() {
    let shared = ArcShared::new(1_u32);
    assert_eq!(ArcShared::new(2_u32).try_unwrap().unwrap(), 2);
    let clone = shared.clone();
    assert!(clone.try_unwrap().is_err());
  }

  #[test]
  fn arc_shared_queue_handle_storage_access() {
    let ring = RingBuffer::<u32>::new(1).with_dynamic(false);
    let storage = ArcShared::new(Mutex::new(ring));
    let handle = storage.storage();
    assert_eq!(handle.lock().unwrap().capacity().to_usize(), 1);
  }

  #[test]
  fn arc_shared_conversions_round_trip() {
    let arc = Arc::new(7_u32);
    let shared = ArcShared::from_arc(arc.clone());
    assert!(Arc::ptr_eq(&shared.clone().into_arc(), &arc));
  }
}
