use super::{queue_error::QueueError, queue_size::QueueSize, QueueStorage};
use crate::sync::Shared;

pub trait QueueBase<E> {
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;
}

pub trait QueueWriter<E>: QueueBase<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>>;
}

pub trait QueueReader<E>: QueueBase<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up_mut(&mut self);
}

pub trait QueueRw<E>: QueueBase<E> {
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up(&self);
}

pub trait QueueHandle<E>: Shared<Self::Storage> + Clone {
  type Storage: QueueStorage<E> + ?Sized;

  fn storage(&self) -> &Self::Storage;
}
