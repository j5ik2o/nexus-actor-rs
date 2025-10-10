//! Error types used across the serialization core.

#[cfg(feature = "alloc")]
use alloc::string::String;

/// Error returned when a value cannot be serialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerializationError {
  /// The requested type is not supported by the serializer implementation.
  UnsupportedType {
    /// Fully-qualified name of the unsupported type.
    #[cfg(feature = "alloc")]
    type_name: String,
  },
  /// Additional error reported by the serializer implementation.
  #[cfg(feature = "alloc")]
  Custom(String),
}

impl SerializationError {
  #[cfg(feature = "alloc")]
  /// Constructs a custom serialization error from the provided message.
  #[must_use]
  pub fn custom(message: String) -> Self {
    SerializationError::Custom(message)
  }

  #[cfg(not(feature = "alloc"))]
  /// Constructs a placeholder error when allocation is unavailable.
  #[must_use]
  pub const fn custom(_: &str) -> Self {
    SerializationError::UnsupportedType {}
  }
}

impl core::fmt::Display for SerializationError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      SerializationError::UnsupportedType { .. } => {
        #[cfg(feature = "alloc")]
        {
          let SerializationError::UnsupportedType { type_name } = self else {
            unreachable!();
          };
          write!(f, "unsupported type: {type_name}")
        }
        #[cfg(not(feature = "alloc"))]
        {
          f.write_str("unsupported type")
        }
      }
      #[cfg(feature = "alloc")]
      SerializationError::Custom(message) => f.write_str(message),
    }
  }
}

/// Error returned when a serialized payload cannot be converted back to its logical representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeserializationError {
  /// The payload contains malformed data.
  MalformedPayload,
  /// The serializer implementation reported a custom error.
  #[cfg(feature = "alloc")]
  Custom(String),
}

impl DeserializationError {
  #[cfg(feature = "alloc")]
  /// Constructs a custom deserialization error from the provided message.
  #[must_use]
  pub fn custom(message: String) -> Self {
    DeserializationError::Custom(message)
  }
}

impl core::fmt::Display for DeserializationError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      DeserializationError::MalformedPayload => f.write_str("malformed payload"),
      #[cfg(feature = "alloc")]
      DeserializationError::Custom(message) => f.write_str(message),
    }
  }
}

/// Errors that can occur while modifying a serializer registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
  /// A serializer with the same identifier has already been registered.
  DuplicateEntry(crate::id::SerializerId),
}

impl core::fmt::Display for RegistryError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      RegistryError::DuplicateEntry(id) => write!(f, "serializer id {id} already registered"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for SerializationError {}

#[cfg(feature = "std")]
impl std::error::Error for DeserializationError {}

#[cfg(feature = "std")]
impl std::error::Error for RegistryError {}
