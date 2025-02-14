use super::serialization::{MessageSerializer, SerializationError};
use super::Message;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

pub struct JsonSerializer<T> {
  _phantom: PhantomData<T>,
}

impl<T> JsonSerializer<T> {
  pub fn new() -> Self {
    Self { _phantom: PhantomData }
  }
}

impl<T> Default for JsonSerializer<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> MessageSerializer for JsonSerializer<T>
where
  T: Message + Serialize + for<'de> Deserialize<'de>,
{
  fn serialize(&self, message: &dyn Message) -> Result<Vec<u8>, SerializationError> {
    message
      .as_any()
      .downcast_ref::<T>()
      .ok_or_else(|| SerializationError::SerializationFailed("Invalid message type".to_string()))
      .and_then(|m| serde_json::to_vec(m).map_err(|e| SerializationError::SerializationFailed(e.to_string())))
  }

  fn deserialize(&self, bytes: &[u8]) -> Result<Box<dyn Message>, SerializationError> {
    serde_json::from_slice::<T>(bytes)
      .map(|m| Box::new(m) as Box<dyn Message>)
      .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
  }

  fn get_format_id(&self) -> u32 {
    2 // JSON format ID
  }
}
