#![cfg(feature = "alloc")]

use alloc::string::String;
use alloc::sync::Arc;

use crate::actor::core_types::message::Message;

/// シリアライズ処理で発生し得るエラー種別。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSerializationError {
  SerializationError(String),
  DeserializationError(String),
  UnknownType,
}

impl CoreSerializationError {
  #[must_use]
  pub fn serialization(msg: impl Into<String>) -> Self {
    Self::SerializationError(msg.into())
  }

  #[must_use]
  pub fn deserialization(msg: impl Into<String>) -> Self {
    Self::DeserializationError(msg.into())
  }

  #[must_use]
  pub const fn unknown_type() -> Self {
    Self::UnknownType
  }
}

impl core::fmt::Display for CoreSerializationError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      CoreSerializationError::SerializationError(msg) => write!(f, "serialization error: {}", msg),
      CoreSerializationError::DeserializationError(msg) => write!(f, "deserialization error: {}", msg),
      CoreSerializationError::UnknownType => write!(f, "unknown serializer type"),
    }
  }
}

/// コア層で共有する RootSerializable 抽象。
pub trait CoreRootSerializable: Message {
  fn serialize(&self) -> Result<Arc<dyn CoreRootSerialized>, CoreSerializationError>;
}

/// コア層で共有する RootSerialized 抽象。
pub trait CoreRootSerialized: Message {
  fn deserialize(&self) -> Result<Arc<dyn CoreRootSerializable>, CoreSerializationError>;
}
