use std::error::Error;
use std::fmt::{Debug, Display};

#[derive(Debug, Clone)]
pub struct ErrorReason {
  message: String,
  cause: Option<Box<dyn Error + Send + Sync>>,
}

impl ErrorReason {
  pub fn new<E: Error + Send + Sync + 'static>(error: E) -> Self {
    Self {
      message: error.to_string(),
      cause: Some(Box::new(error)),
    }
  }

  pub fn message(&self) -> &str {
    &self.message
  }

  pub fn cause(&self) -> Option<&(dyn Error + Send + Sync)> {
    self.cause.as_deref()
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
