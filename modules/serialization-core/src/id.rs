//! Definition of serializer identifiers and reserved ranges.

/// Identifier used to select a serializer implementation at runtime.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SerializerId(u32);

impl SerializerId {
  /// Creates a new identifier from the provided integer value.
  #[inline]
  #[must_use]
  pub const fn new(raw: u32) -> Self {
    Self(raw)
  }

  /// Returns the underlying integer value.
  #[inline]
  #[must_use]
  pub const fn value(self) -> u32 {
    self.0
  }
}

impl From<SerializerId> for u32 {
  fn from(value: SerializerId) -> Self {
    value.value()
  }
}

impl core::fmt::Display for SerializerId {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{}", self.0)
  }
}

/// Serializer IDs below this value are reserved for framework provided serializers.
pub const USER_DEFINED_START: u32 = 1024;

/// Reserved identifier for a no-op serializer used in tests.
pub const TEST_ECHO_SERIALIZER_ID: SerializerId = SerializerId(0);
