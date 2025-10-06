use super::*;
use crate::generated::cluster::PubSubEnvelope;
use crate::serializer::initialize_proto_serializers;
use nexus_actor_std_rs::actor::message::WatchMessage;
use nexus_actor_std_rs::generated::actor::{Pid, Terminated, TerminatedReason, Watch};
use prost::Message as ProstMessage;

#[derive(Clone, PartialEq, ::prost::Message, nexus_message_derive_rs::Message)]
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
  let decoded =
    decode_wire_message(std::any::type_name::<Terminated>(), &SerializerId::Proto, &bytes).expect("decode terminated");

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
  let decoded =
    decode_wire_message(std::any::type_name::<Watch>(), &SerializerId::Proto, &bytes).expect("decode watch");

  match decoded {
    DecodedMessage::System(SystemMessage::Watch(actual)) => {
      let expected = WatchMessage::new(Pid::default().to_core());
      assert_eq!(actual, expected);
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
  let decoded = decode_wire_message(std::any::type_name::<PlainMessage>(), &SerializerId::Proto, &bytes)
    .expect("decode plain message");

  match decoded {
    DecodedMessage::User(message) => {
      assert_eq!(message.get_type_name(), std::any::type_name::<PlainMessage>());
    }
    other => panic!("expected user message, got {other:?}"),
  }
}
