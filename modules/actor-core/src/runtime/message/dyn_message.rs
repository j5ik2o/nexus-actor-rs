use alloc::boxed::Box;
use core::any::{Any, TypeId};
use core::fmt::{self, Debug};

use nexus_utils_core_rs::Element;

/// Type-erased message used internally by the runtime.
pub struct DynMessage {
  inner: Box<dyn Any + Send + Sync>,
}

impl DynMessage {
  /// Creates a `DynMessage` wrapping an arbitrary value.
  pub fn new<T>(value: T) -> Self
  where
    T: Any + Send + Sync,
  {
    Self { inner: Box::new(value) }
  }

  /// Gets the `TypeId` of the internally held value.
  pub fn type_id(&self) -> TypeId {
    self.inner.as_ref().type_id()
  }

  /// Attempts to downcast to type T by moving ownership.
  pub fn downcast<T>(self) -> Result<T, Self>
  where
    T: Any + Send + Sync,
  {
    match self.inner.downcast::<T>() {
      Ok(boxed) => Ok(*boxed),
      Err(inner) => Err(Self { inner }),
    }
  }

  /// Attempts to downcast to type T through a reference.
  pub fn downcast_ref<T>(&self) -> Option<&T>
  where
    T: Any + Send + Sync,
  {
    self.inner.downcast_ref::<T>()
  }

  /// Attempts to downcast to type T through a mutable reference.
  pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
  where
    T: Any + Send + Sync,
  {
    self.inner.downcast_mut::<T>()
  }

  /// Extracts the internal type-erased value.
  pub fn into_any(self) -> Box<dyn Any + Send + Sync> {
    self.inner
  }
}

impl Debug for DynMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DynMessage<{}>", core::any::type_name::<Self>())
  }
}

impl Element for DynMessage {}
