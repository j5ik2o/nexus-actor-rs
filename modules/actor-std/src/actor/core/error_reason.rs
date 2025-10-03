use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

use backtrace::Backtrace;
use nexus_actor_core_rs::error::{ErrorReasonCore, TakeErrorCore};

pub type TakeError = TakeErrorCore;

#[derive(Clone)]
pub struct ErrorReason {
  inner: ErrorReasonCore,
  backtrace: Backtrace,
}

impl ErrorReason {
  pub fn new<T>(reason: T, code: i32) -> Self
  where
    T: Send + Sync + 'static, {
    Self {
      inner: ErrorReasonCore::new(reason, code),
      backtrace: Backtrace::new(),
    }
  }

  pub fn backtrace(&self) -> &Backtrace {
    &self.backtrace
  }

  pub fn is_type<T: Send + Sync + 'static>(&self) -> bool {
    self.inner.is_type::<T>()
  }

  pub fn take<T>(&mut self) -> Result<T, TakeError>
  where
    T: Send + Sync + 'static, {
    self.inner.take()
  }

  pub fn take_or_panic<T>(&mut self) -> T
  where
    T: Send + Sync + 'static, {
    self.inner.take_or_panic()
  }

  pub fn code(&self) -> i32 {
    self.inner.code
  }

  pub fn as_core(&self) -> &ErrorReasonCore {
    &self.inner
  }

  pub fn from_core(inner: ErrorReasonCore) -> Self {
    Self {
      inner,
      backtrace: Backtrace::new(),
    }
  }

  pub fn into_core(self) -> ErrorReasonCore {
    self.inner
  }
}

impl From<std::io::Error> for ErrorReason {
  fn from(error: std::io::Error) -> Self {
    Self::from_core(ErrorReasonCore::new(error, 0))
  }
}

impl From<String> for ErrorReason {
  fn from(value: String) -> Self {
    Self::from_core(ErrorReasonCore::from(value))
  }
}

impl From<&str> for ErrorReason {
  fn from(value: &str) -> Self {
    Self::from_core(ErrorReasonCore::from(value))
  }
}

impl Display for ErrorReason {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Display::fmt(&self.inner, f)
  }
}

impl Debug for ErrorReason {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ErrorReason")
      .field("inner", &self.inner)
      .field("backtrace", &self.backtrace)
      .finish()
  }
}

impl Error for ErrorReason {}

impl PartialEq for ErrorReason {
  fn eq(&self, other: &Self) -> bool {
    self.inner == other.inner
  }
}

impl Eq for ErrorReason {}

impl std::hash::Hash for ErrorReason {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner.hash(state);
    std::ptr::addr_of!(self.backtrace).hash(state);
  }
}

static_assertions::assert_impl_all!(ErrorReason: Send, Sync);
