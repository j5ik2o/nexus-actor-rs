//! Serialized message representation shared by serializers.

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::id::SerializerId;

/// Key/value metadata attached to a serialized payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageHeader {
  /// Header name.
  #[cfg(feature = "alloc")]
  pub key: String,
  /// Header value.
  #[cfg(feature = "alloc")]
  pub value: String,
}

impl MessageHeader {
  /// Creates a new header entry.
  #[cfg(feature = "alloc")]
  #[inline]
  #[must_use]
  pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      key: key.into(),
      value: value.into(),
    }
  }
}

/// Container produced by serializers containing the encoded payload and associated metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializedMessage {
  /// Identifier of the serializer that produced the payload.
  pub serializer_id: SerializerId,
  /// Fully-qualified type name, when known.
  #[cfg(feature = "alloc")]
  pub type_name: Option<String>,
  /// Raw payload bytes.
  #[cfg(feature = "alloc")]
  pub payload: Vec<u8>,
  /// Additional metadata headers.
  #[cfg(feature = "alloc")]
  pub headers: Vec<MessageHeader>,
}

impl SerializedMessage {
  /// Constructs a new serialized message for the specified serializer.
  #[cfg(feature = "alloc")]
  #[must_use]
  pub fn new(serializer_id: SerializerId, payload: Vec<u8>) -> Self {
    Self {
      serializer_id,
      type_name: None,
      payload,
      headers: Vec::new(),
    }
  }

  /// Assigns a type name to the message.
  #[cfg(feature = "alloc")]
  pub fn with_type_name(mut self, type_name: impl Into<String>) -> Self {
    self.type_name = Some(type_name.into());
    self
  }

  /// Updates the stored type name.
  #[cfg(feature = "alloc")]
  pub fn set_type_name(&mut self, type_name: impl Into<String>) {
    self.type_name = Some(type_name.into());
  }

  /// Adds a metadata header to the message.
  #[cfg(feature = "alloc")]
  pub fn push_header(&mut self, key: impl Into<String>, value: impl Into<String>) {
    self.headers.push(MessageHeader::new(key, value));
  }
}
