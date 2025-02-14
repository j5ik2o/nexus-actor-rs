use super::serialization::{MessageSerializer, SerializationError};
use super::Message;
use prost::Message as ProstMessage;
use std::marker::PhantomData;

pub struct ProtoSerializer<T> {
  _phantom: PhantomData<T>,
}

impl<T> ProtoSerializer<T> {
  pub fn new() -> Self {
    Self { _phantom: PhantomData }
  }
}

impl<T> Default for ProtoSerializer<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> MessageSerializer for ProtoSerializer<T>
where
  T: Message + ProstMessage + Default,
{
  fn serialize(&self, message: &dyn Message) -> Result<Vec<u8>, SerializationError> {
    message
      .as_any()
      .downcast_ref::<T>()
      .ok_or_else(|| SerializationError::SerializationFailed("Invalid message type".to_string()))
      .map(|m| m.encode_to_vec())
  }

  fn deserialize(&self, bytes: &[u8]) -> Result<Box<dyn Message>, SerializationError> {
    T::decode(bytes)
      .map(|m| Box::new(m) as Box<dyn Message>)
      .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
  }

  fn get_format_id(&self) -> u32 {
    1 // Proto format ID
  }
}
