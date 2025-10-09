/// Enumeration representing the size limit of a queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSize {
  /// No limit (unlimited).
  Limitless,
  /// Limited to the specified size.
  Limited(usize),
}

impl QueueSize {
  /// Constant constructor representing an unlimited size queue.
  pub const fn limitless() -> Self {
    Self::Limitless
  }

  /// Constant constructor representing a queue limited to the specified size.
  pub const fn limited(value: usize) -> Self {
    Self::Limited(value)
  }

  /// Determines whether this size is unlimited.
  pub const fn is_limitless(&self) -> bool {
    matches!(self, Self::Limitless)
  }

  /// Gets the size as `usize`. Returns `usize::MAX` if unlimited.
  pub const fn to_usize(self) -> usize {
    match self {
      Self::Limitless => usize::MAX,
      Self::Limited(value) => value,
    }
  }
}

impl Default for QueueSize {
  fn default() -> Self {
    QueueSize::limited(0)
  }
}

#[cfg(test)]
mod tests {
  use super::QueueSize;

  #[test]
  fn queue_size_helpers_work_as_expected() {
    let zero = QueueSize::limited(0);
    let limitless = QueueSize::limitless();

    assert!(!zero.is_limitless());
    assert_eq!(zero.to_usize(), 0);

    assert!(limitless.is_limitless());
    assert_eq!(limitless.to_usize(), usize::MAX);

    match limitless {
      QueueSize::Limitless => {}
      _ => panic!("expected limitless variant"),
    }

    match zero {
      QueueSize::Limited(value) => assert_eq!(value, 0),
      _ => panic!("expected limited variant"),
    }
  }
}
