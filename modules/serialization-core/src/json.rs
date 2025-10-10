#![cfg(feature = "serde-json")]

//! Serializer implementation backed by `serde_json`.

use crate::error::{DeserializationError, SerializationError};
use crate::id::SerializerId;
use crate::message::SerializedMessage;
use crate::serializer::Serializer;
use alloc::vec::Vec;
use nexus_utils_core_rs::sync::ArcShared;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Serializer identifier reserved for serde_json.
pub const SERDE_JSON_SERIALIZER_ID: SerializerId = SerializerId::new(1);

/// Serializer implementation backed by `serde_json`.
#[derive(Debug, Clone, Default)]
pub struct SerdeJsonSerializer;

impl SerdeJsonSerializer {
  /// Creates a new instance of the serde_json serializer.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Serializes the provided value and returns a [`SerializedMessage`].
  pub fn serialize_value<T>(
    &self,
    type_name: Option<&str>,
    value: &T,
  ) -> Result<SerializedMessage, SerializationError>
  where
    T: Serialize, {
    let payload = serde_json::to_vec(value).map_err(|err| SerializationError::custom(err.to_string()))?;
    self.serialize_with_type_name_opt(payload.as_slice(), type_name)
  }

  /// Deserializes the provided message into the requested type.
  pub fn deserialize_value<T>(&self, message: &SerializedMessage) -> Result<T, DeserializationError>
  where
    T: DeserializeOwned, {
    serde_json::from_slice(&message.payload).map_err(|err| DeserializationError::custom(err.to_string()))
  }
}

impl Serializer for SerdeJsonSerializer {
  fn serializer_id(&self) -> SerializerId {
    SERDE_JSON_SERIALIZER_ID
  }

  fn content_type(&self) -> &str {
    "application/json"
  }

  fn serialize_with_type_name_opt(
    &self,
    payload: &[u8],
    type_name: Option<&str>,
  ) -> Result<SerializedMessage, SerializationError> {
    serde_json::from_slice::<serde_json::Value>(payload).map_err(|err| SerializationError::custom(err.to_string()))?;
    let mut message = SerializedMessage::new(self.serializer_id(), payload.to_vec());
    if let Some(name) = type_name {
      message.set_type_name(name);
    }
    Ok(message)
  }

  fn deserialize(&self, message: &SerializedMessage) -> Result<Vec<u8>, DeserializationError> {
    serde_json::from_slice::<serde_json::Value>(&message.payload)
      .map_err(|err| DeserializationError::custom(err.to_string()))?;
    Ok(message.payload.clone())
  }
}

/// Convenience wrapper that produces an `ArcShared` serializer instance.
#[must_use]
pub fn shared_json_serializer() -> ArcShared<SerdeJsonSerializer> {
  ArcShared::new(SerdeJsonSerializer::new())
}
