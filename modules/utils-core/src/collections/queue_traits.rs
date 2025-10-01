use core::fmt::Debug;

use super::Element;
use super::{QueueError, QueueSize};

pub trait QueueSupport {}

pub trait QueueBase<E: Element>: Debug + Send + Sync {
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;

  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  fn is_full(&self) -> bool {
    self.capacity() == self.len()
  }

  fn non_empty(&self) -> bool {
    !self.is_empty()
  }

  fn non_full(&self) -> bool {
    !self.is_full()
  }
}

pub trait QueueWriter<E: Element>: QueueBase<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>>;

  fn offer_all<I>(&mut self, elements: I) -> Result<(), QueueError<E>>
  where
    I: IntoIterator<Item = E>, {
    for element in elements {
      self.offer(element)?;
    }
    Ok(())
  }
}

pub trait QueueReader<E: Element>: QueueBase<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up(&mut self);
}
