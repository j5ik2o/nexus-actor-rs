use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricsError {
  DuplicateMetricKey(String),
}

impl Display for MetricsError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      MetricsError::DuplicateMetricKey(key) => write!(f, "metrics key '{}' already exists", key),
    }
  }
}

impl Error for MetricsError {}
