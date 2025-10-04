use super::*;
use nexus_actor_std_rs::actor::message::Message;
use nexus_message_derive_rs::Message;
use std::env;
use tracing_subscriber::EnvFilter;

#[derive(Clone, PartialEq, Message, ::prost::Message, Serialize, Deserialize)]
pub struct TestMessage {
  #[prost(int32, tag = "1")]
  pub a: i32,
  #[prost(string, tag = "2")]
  pub b: String,
}

#[test]
fn test_proto_serialization() {
  initialize_serializers::<TestMessage>().expect("Failed to register serializer");
  let msg = TestMessage {
    a: 42,
    b: "world".to_string(),
  };
  let bytes = serialize(&msg, &SerializerId::Proto).unwrap();
  let deserialized = deserialize::<TestMessage>(&bytes, &SerializerId::Proto).unwrap();
  assert_eq!(msg, deserialized);
}

#[test]
fn test_proto_serialization_any() {
  env::set_var("RUST_LOG", "nexus_actor_std_rs=info");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  initialize_proto_serializers::<TestMessage>().expect("Failed to register serializer");
  let msg = TestMessage {
    a: 42,
    b: "world".to_string(),
  };
  let bytes = serialize_any(&msg, &SerializerId::Proto, std::any::type_name::<TestMessage>()).unwrap();
  // let deserialized = deserialize_any(&bytes, &SerializerId::Proto, std::any::type_name::<TestMessage>()).unwrap();
  let _deserialized = deserialize::<TestMessage>(&bytes, &SerializerId::Proto).unwrap();
  // assert_eq!(msg, deserialized);
}

#[test]
fn test_json_serialization() {
  initialize_serializers::<TestMessage>().expect("Failed to register serializer");
  let msg = TestMessage {
    a: 42,
    b: "hello".to_string(),
  };
  let bytes = serialize(&msg, &SerializerId::Json).unwrap();
  let deserialized = deserialize::<TestMessage>(&bytes, &SerializerId::Json).unwrap();
  assert_eq!(msg, deserialized);
}
