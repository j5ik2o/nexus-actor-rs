use crate::sync::Shared;

use super::storage::QueueStorage;

pub trait SharedQueueHandle<E>: Shared<Self::Storage> + Clone {
  type Storage: QueueStorage<E> + ?Sized;

  fn storage(&self) -> &Self::Storage;
}
