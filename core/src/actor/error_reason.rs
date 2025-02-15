use std::backtrace::Backtrace;
use std::error::Error;
use std::fmt::{Debug, Display};

#[derive(Debug, Clone)]
pub struct ErrorReason {
  message: String,
  cause: Option<Box<dyn Error + Send + Sync>>,
  backtrace: Backtrace,
}

impl ErrorReason {
  pub fn new<E: Error + Send + Sync + 'static>(error: E) -> Self {
    Self {
      message: error.to_string(),
      cause: Some(Box::new(error)),
      backtrace: Backtrace::capture(),
    }
  }

  pub fn message(&self) -> &str {
    &self.message
  }

  pub fn cause(&self) -> Option<&(dyn Error + Send + Sync)> {
    self.cause.as_deref()
  }

  pub fn reason(&self) -> &ErrorReason {
    self
  }

  pub fn backtrace(&self) -> &Backtrace {
    &self.backtrace
  }
}

impl Display for ErrorReason {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl Error for ErrorReason {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.cause.as_deref()
  }
}

pub type ErrorReasonType = Box<dyn Error + Send + Sync>;
