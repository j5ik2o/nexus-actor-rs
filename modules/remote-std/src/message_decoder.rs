use std::sync::Arc;

use crate::cluster::messages::{DeliverBatchRequest, PubSubAutoResponseBatch, PubSubBatch};
use crate::generated::cluster::{DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport};
use crate::serializer::{
  deserialize_any, deserialize_message, AnyDowncastExt, RootSerializable, RootSerialized, SerializerError, SerializerId,
};
use nexus_actor_std_rs::actor::message::{Message, SystemMessage, SystemMessageFromProtoExt};
use nexus_actor_std_rs::generated::actor::{Stop, Terminated, Unwatch, Watch};
use prost::Message as ProstMessage;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub enum DecodedMessage {
  Terminated(Terminated),
  System(SystemMessage),
  User(Arc<dyn Message>),
}

fn downcast_root_serializable<T>(value: Arc<dyn RootSerializable>) -> Result<T, SerializerError>
where
  T: Clone + Message + 'static, {
  let any = value.as_any();
  any
    .downcast_ref::<T>()
    .cloned()
    .ok_or_else(|| SerializerError::deserialization("Failed to downcast RootSerializable"))
}

fn deserialize_root_serializable<TTransport, TResult>(
  bytes: &[u8],
  serializer_id: &SerializerId,
  type_name: &str,
) -> Result<Arc<dyn Message>, SerializerError>
where
  TTransport: Clone + RootSerialized + Send + Sync + 'static,
  TResult: Clone + Message + 'static, {
  let transport_arc = deserialize_any(bytes, serializer_id, type_name)?;
  let transport = transport_arc
    .downcast_arc::<TTransport>()
    .map_err(|_| SerializerError::deserialization("Failed to downcast transport"))?;

  let result = transport.as_ref().deserialize()?;
  let typed = downcast_root_serializable::<TResult>(result)?;
  Ok(Arc::new(typed) as Arc<dyn Message>)
}

pub fn decode_wire_message(
  type_name: &str,
  serializer_id: &SerializerId,
  payload: &[u8],
) -> Result<DecodedMessage, SerializerError> {
  if type_name == std::any::type_name::<PubSubBatchTransport>() {
    let message =
      deserialize_root_serializable::<PubSubBatchTransport, PubSubBatch>(payload, serializer_id, type_name)?;
    return Ok(DecodedMessage::User(message));
  }

  if type_name == std::any::type_name::<DeliverBatchRequestTransport>() {
    let message = deserialize_root_serializable::<DeliverBatchRequestTransport, DeliverBatchRequest>(
      payload,
      serializer_id,
      type_name,
    )?;
    return Ok(DecodedMessage::User(message));
  }

  if type_name == std::any::type_name::<PubSubAutoRespondBatchTransport>() {
    let message = deserialize_root_serializable::<PubSubAutoRespondBatchTransport, PubSubAutoResponseBatch>(
      payload,
      serializer_id,
      type_name,
    )?;
    return Ok(DecodedMessage::User(message));
  }

  if type_name == std::any::type_name::<Terminated>() {
    let terminated = Terminated::decode(payload).map_err(|e| SerializerError::deserialization(e.to_string()))?;
    return Ok(DecodedMessage::Terminated(terminated));
  }

  if type_name == std::any::type_name::<Stop>() {
    return Ok(DecodedMessage::System(SystemMessage::stop()));
  }

  if type_name == std::any::type_name::<Watch>() {
    let watch = Watch::decode(payload).map_err(|e| SerializerError::deserialization(e.to_string()))?;
    let system_message = watch
      .to_system_message()
      .ok_or_else(|| SerializerError::deserialization("Watch message missing watcher"))?;
    return Ok(DecodedMessage::System(system_message));
  }

  if type_name == std::any::type_name::<Unwatch>() {
    let unwatch = Unwatch::decode(payload).map_err(|e| SerializerError::deserialization(e.to_string()))?;
    let system_message = unwatch
      .to_system_message()
      .ok_or_else(|| SerializerError::deserialization("Unwatch message missing watcher"))?;
    return Ok(DecodedMessage::System(system_message));
  }

  let message_arc = deserialize_message(payload, serializer_id, type_name)?;
  Ok(DecodedMessage::User(message_arc))
}
