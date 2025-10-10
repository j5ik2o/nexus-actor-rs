//! Serializer trait defining the boundary between type systems and raw payloads.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::error::{DeserializationError, SerializationError};
use crate::id::SerializerId;
use crate::message::SerializedMessage;

/// Abstraction implemented by concrete serializer backends.
pub trait Serializer: Send + Sync {
  /// Returns the unique identifier associated with this serializer.
  fn serializer_id(&self) -> SerializerId;

  /// Describes the wire-level content type produced by this serializer (e.g. `application/json`).
  fn content_type(&self) -> &str;

  /// Serializes the provided payload bytes into a [`SerializedMessage`].
  #[cfg(feature = "alloc")]
  fn serialize(&self, payload: &[u8]) -> Result<SerializedMessage, SerializationError> {
    self.serialize_with_type_name_opt(payload, None)
  }

  /// Serializes the payload bytes and attaches an explicit type name.
  #[cfg(feature = "alloc")]
  fn serialize_with_type_name(&self, payload: &[u8], type_name: &str) -> Result<SerializedMessage, SerializationError> {
    self.serialize_with_type_name_opt(payload, Some(type_name))
  }

  /// Core serialization entry point used by the default helper methods above.
  #[cfg(feature = "alloc")]
  fn serialize_with_type_name_opt(
    &self,
    payload: &[u8],
    type_name: Option<&str>,
  ) -> Result<SerializedMessage, SerializationError>;

  /// Restores payload bytes previously emitted by [`Serializer::serialize`].
  #[cfg(feature = "alloc")]
  fn deserialize(&self, message: &SerializedMessage) -> Result<Vec<u8>, DeserializationError>;
}
