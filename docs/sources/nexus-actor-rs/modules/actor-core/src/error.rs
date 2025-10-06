#![cfg(feature = "alloc")]

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use core::any::{Any, TypeId};
use core::fmt::{Debug, Display, Formatter};

#[derive(Clone)]
pub struct ErrorReasonCore {
  reason: Option<Arc<dyn Any + Send + Sync>>,
  pub code: i32,
}

impl ErrorReasonCore {
  pub fn new<T>(reason: T, code: i32) -> Self
  where
    T: Send + Sync + 'static, {
    Self {
      reason: Some(Arc::new(reason)),
      code,
    }
  }

  pub fn is_type<T: Send + Sync + 'static>(&self) -> bool {
    self.reason.as_ref().map_or(false, |inner| inner.is::<T>())
  }

  pub fn take<T>(&mut self) -> Result<T, TakeErrorCore>
  where
    T: Send + Sync + 'static, {
    match self.reason.take() {
      Some(value) => match Arc::downcast::<T>(value) {
        Ok(typed) => match Arc::try_unwrap(typed) {
          Ok(v) => Ok(v),
          Err(shared) => {
            let trait_obj: Arc<dyn Any + Send + Sync> = shared;
            self.reason = Some(trait_obj);
            Err(TakeErrorCore::StillShared)
          }
        },
        Err(original) => {
          let found = original.type_id();
          self.reason = Some(original);
          Err(TakeErrorCore::TypeMismatch {
            expected: core::any::type_name::<T>(),
            found,
          })
        }
      },
      None => Err(TakeErrorCore::AlreadyTaken),
    }
  }

  pub fn take_or_panic<T>(&mut self) -> T
  where
    T: Send + Sync + 'static, {
    self
      .take()
      .unwrap_or_else(|err| panic!("Failed to take error: {:?}", err))
  }

  pub fn reason_type_id(&self) -> Option<TypeId> {
    self.reason.as_ref().map(|inner| inner.type_id())
  }
}

impl Default for ErrorReasonCore {
  fn default() -> Self {
    Self { reason: None, code: 0 }
  }
}

impl Debug for ErrorReasonCore {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ErrorReasonCore")
      .field("reason_type", &self.reason_type_id())
      .field("code", &self.code)
      .finish()
  }
}

impl Display for ErrorReasonCore {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    match self.reason_type_id() {
      Some(ty) => write!(f, "ErrorReasonCore(type_id={:?}, code={})", ty, self.code),
      None => write!(f, "ErrorReasonCore(code={}, empty)", self.code),
    }
  }
}

impl PartialEq for ErrorReasonCore {
  fn eq(&self, other: &Self) -> bool {
    match (&self.reason, &other.reason) {
      (Some(a), Some(b)) => Arc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    }
  }
}

impl Eq for ErrorReasonCore {}

impl core::hash::Hash for ErrorReasonCore {
  fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
    self.reason.is_some().hash(state);
    if let Some(reason) = &self.reason {
      reason.type_id().hash(state);
    }
    self.code.hash(state);
  }
}

#[derive(Debug)]
pub enum TakeErrorCore {
  TypeMismatch { expected: &'static str, found: TypeId },
  StillShared,
  AlreadyTaken,
}

impl From<&str> for ErrorReasonCore {
  fn from(value: &str) -> Self {
    Self::new(value.to_owned(), 0)
  }
}

impl From<String> for ErrorReasonCore {
  fn from(value: String) -> Self {
    Self::new(value, 0)
  }
}
