use std::any::Any;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use backtrace::Backtrace;

#[derive(Clone)]
pub struct ErrorReason {
  reason: Option<Arc<dyn Any + Send + Sync>>,
  pub(crate) code: i32,
  backtrace: Backtrace,
}

impl ErrorReason {
  pub fn new<T>(reason: T, code: i32) -> Self
  where
    T: Send + Sync + 'static, {
    Self {
      reason: Some(Arc::new(reason)),
      code,
      backtrace: Backtrace::new(),
    }
  }

  pub fn backtrace(&self) -> &Backtrace {
    &self.backtrace
  }

  pub fn is_type<T: Send + Sync + 'static>(&self) -> bool {
    match self.reason.as_ref() {
      Some(m) => m.is::<T>(),
      None => false,
    }
  }

  pub fn take<T>(&mut self) -> Result<T, TakeError>
  where
    T: Send + Sync + 'static, {
    match self.reason.take() {
      Some(v) => match v.downcast::<T>() {
        Ok(arc_v) => match Arc::try_unwrap(arc_v) {
          Ok(v) => Ok(v),
          Err(arc_v) => {
            self.reason = Some(arc_v);
            Err(TakeError::StillShared)
          }
        },
        Err(original) => {
          self.reason = Some(original.clone());
          Err(TakeError::TypeMismatch {
            expected: std::any::type_name::<T>(),
            found: original.type_id(),
          })
        }
      },
      None => Err(TakeError::AlreadyTaken),
    }
  }

  pub fn take_or_panic<T>(&mut self) -> T
  where
    T: Error + Send + Sync + 'static, {
    self.take().unwrap_or_else(|e| panic!("Failed to take error: {:?}", e))
  }
}

static_assertions::assert_impl_all!(ErrorReason: Send, Sync);

#[derive(Debug)]
pub enum TakeError {
  TypeMismatch {
    expected: &'static str,
    found: std::any::TypeId,
  },
  StillShared,
  AlreadyTaken,
}

impl Display for ErrorReason {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match &self.reason {
      Some(error) => write!(f, "ActorInnerError: {:?}", error),
      None => write!(f, "ActorInnerError: Error has been taken"),
    }
  }
}

impl Debug for ErrorReason {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorInnerError")
      .field("inner_error", &self.reason)
      .field("backtrace", &self.backtrace)
      .finish()
  }
}

impl Error for ErrorReason {}

impl PartialEq for ErrorReason {
  fn eq(&self, other: &Self) -> bool {
    match (&self.reason, &other.reason) {
      (Some(a), Some(b)) => Arc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    }
  }
}

impl Eq for ErrorReason {}

impl std::hash::Hash for ErrorReason {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.reason.is_some().hash(state);
    if let Some(error) = &self.reason {
      error.type_id().hash(state);
    }
    std::ptr::addr_of!(self.backtrace).hash(state);
  }
}

impl From<std::io::Error> for ErrorReason {
  fn from(error: std::io::Error) -> Self {
    let error_arc = Arc::new(error);
    ErrorReason {
      reason: Some(error_arc.clone()),
      code: 0,
      backtrace: Backtrace::new(),
    }
  }
}

impl From<String> for ErrorReason {
  fn from(s: String) -> Self {
    Self::new(s, 0)
  }
}

impl From<&str> for ErrorReason {
  fn from(s: &str) -> Self {
    Self::new(s.to_string(), 0)
  }
}
