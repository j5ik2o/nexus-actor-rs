use std::sync::Arc;

use crate::cluster::messages::{DeliverBatchRequest, PubSubAutoResponseBatch, PubSubBatch};
use crate::generated::cluster::{
  DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport,
};
use crate::serializer::{
  deserialize_any, deserialize_message, AnyDowncastExt, RootSerializable, RootSerialized, SerializerError,
  SerializerId,
};
use nexus_actor_core_rs::actor::message::{Message, SystemMessage};
use nexus_actor_core_rs::generated::actor::{Stop, Terminated, Unwatch, Watch};
use prost::Message as ProstMessage;

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
    .ok_or_else(|| SerializerError::DeserializationError("Failed to downcast RootSerializable".to_string()))
}

fn deserialize_root_serializable<TTransport, TResult>(
  bytes: &[u8],
  serializer_id: &SerializerId,
  type_name: &str,
) -> Result<Arc<dyn Message>, SerializerError>
where
  TTransport: Clone + RootSerialized + Send + Sync + 'static,
  TResult: Clone + Message + 'static,
{
  let transport_arc = deserialize_any(bytes, serializer_id, type_name)?;
  let transport = transport_arc
    .downcast_arc::<TTransport>()
    .map_err(|_| SerializerError::DeserializationError("Failed to downcast transport".to_string()))?;

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
    let message = deserialize_root_serializable::<PubSubBatchTransport, PubSubBatch>(payload, serializer_id, type_name)?;
    return Ok(DecodedMessage::User(message));
  }

  if type_name == std::any::type_name::<DeliverBatchRequestTransport>() {
    let message =
      deserialize_root_serializable::<DeliverBatchRequestTransport, DeliverBatchRequest>(payload, serializer_id, type_name)?;
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
    let terminated = Terminated::decode(payload)
      .map_err(|e| SerializerError::DeserializationError(e.to_string()))?;
    return Ok(DecodedMessage::Terminated(terminated));
  }

  if type_name == std::any::type_name::<Stop>() {
    return Ok(DecodedMessage::System(SystemMessage::of_stop()));
  }

  if type_name == std::any::type_name::<Watch>() {
    let watch = Watch::decode(payload).map_err(|e| SerializerError::DeserializationError(e.to_string()))?;
    return Ok(DecodedMessage::System(SystemMessage::of_watch(watch)));
  }

  if type_name == std::any::type_name::<Unwatch>() {
    let unwatch = Unwatch::decode(payload).map_err(|e| SerializerError::DeserializationError(e.to_string()))?;
    return Ok(DecodedMessage::System(SystemMessage::of_unwatch(unwatch)));
  }

  let message_arc = deserialize_message(payload, serializer_id, type_name)?;
  Ok(DecodedMessage::User(message_arc))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::generated::cluster::PubSubEnvelope;
  use crate::serializer::initialize_proto_serializers;
  use nexus_actor_core_rs::generated::actor::{Pid, Terminated, TerminatedReason, Watch};
  use prost::Message as ProstMessage;

  #[derive(Clone, PartialEq, ::prost::Message, nexus_actor_message_derive_rs::Message)]
  struct PlainMessage {
    #[prost(string, tag = "1")]
    payload: String,
  }

  fn setup_serializers() {
    let _ = initialize_proto_serializers::<PubSubBatchTransport>();
    let _ = initialize_proto_serializers::<DeliverBatchRequestTransport>();
    let _ = initialize_proto_serializers::<PubSubAutoRespondBatchTransport>();
  }

  #[test]
  fn decode_terminated_message() {
    setup_serializers();
    let terminated = Terminated {
      who: Some(Pid::default()),
      why: TerminatedReason::Stopped as i32,
    };
    let bytes = terminated.encode_to_vec();
    let decoded = decode_wire_message(
      std::any::type_name::<Terminated>(),
      &SerializerId::Proto,
      &bytes,
    )
    .expect("decode terminated");

    match decoded {
      DecodedMessage::Terminated(value) => {
        assert_eq!(value.why, TerminatedReason::Stopped as i32);
      }
      other => panic!("expected terminated, got {other:?}"),
    }
  }

  #[test]
  fn decode_watch_to_system_message() {
    setup_serializers();
    let watch = Watch {
      watcher: Some(Pid::default()),
    };
    let bytes = watch.encode_to_vec();
    let decoded = decode_wire_message(
      std::any::type_name::<Watch>(),
      &SerializerId::Proto,
      &bytes,
    )
    .expect("decode watch");

    match decoded {
      DecodedMessage::System(SystemMessage::Watch(actual)) => {
        assert!(actual.watcher.is_some());
      }
      other => panic!("expected system message, got {other:?}"),
    }
  }

  #[test]
  fn decode_pubsub_batch_transport_returns_user_message() {
    setup_serializers();

    let payload = PlainMessage {
      payload: "pubsub".to_string(),
    };
    let _ = initialize_proto_serializers::<PlainMessage>();
    let message_bytes = payload.encode_to_vec();

    let transport = PubSubBatchTransport {
      type_names: vec![std::any::type_name::<PlainMessage>().to_string()],
      envelopes: vec![PubSubEnvelope {
        type_id: 0,
        message_data: message_bytes,
        serializer_id: u32::from(SerializerId::Proto) as i32,
      }],
    };

    let bytes = transport.encode_to_vec();
    let decoded = decode_wire_message(
      std::any::type_name::<PubSubBatchTransport>(),
      &SerializerId::Proto,
      &bytes,
    )
    .expect("decode pubsub batch");

    match decoded {
      DecodedMessage::User(message) => {
        assert_eq!(message.get_type_name(), std::any::type_name::<PubSubBatch>());
      }
      other => panic!("expected user message, got {other:?}"),
    }
  }

  #[test]
  fn decode_regular_message_returns_user() {
    setup_serializers();
    let _ = initialize_proto_serializers::<PlainMessage>();

    let msg = PlainMessage {
      payload: "hello".to_string(),
    };

    let bytes = msg.encode_to_vec();
    let decoded = decode_wire_message(
      std::any::type_name::<PlainMessage>(),
      &SerializerId::Proto,
      &bytes,
    )
    .expect("decode plain message");

    match decoded {
      DecodedMessage::User(message) => {
        assert_eq!(message.get_type_name(), std::any::type_name::<PlainMessage>());
      }
      other => panic!("expected user message, got {other:?}"),
    }
  }
}
