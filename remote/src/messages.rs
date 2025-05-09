use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remote_message::MessageType;
use crate::generated::remote::{
  ClientConnection, ConnectRequest, ConnectResponse, DisconnectRequest, MessageBatch, MessageEnvelope, MessageHeader,
  RemoteMessage, ServerConnection,
};
use nexus_actor_core_rs::actor::message::ReadonlyMessageHeadersHandle;
use nexus_actor_core_rs::actor::message::{Message, MessageHandle};
use nexus_actor_core_rs::generated::actor::Pid;
use nexus_actor_core_rs::Message;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointTerminatedEvent {
  pub address: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointConnectedEvent {
  pub address: String,
}

#[derive(Debug, Clone, PartialEq, Message)]
pub enum EndpointEvent {
  EndpointTerminated(EndpointTerminatedEvent),
  EndpointConnected(EndpointConnectedEvent),
}

impl EndpointEvent {
  pub fn is_connected(&self) -> bool {
    matches!(self, EndpointEvent::EndpointConnected(_))
  }

  pub fn is_terminated(&self) -> bool {
    matches!(self, EndpointEvent::EndpointTerminated(_))
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
pub struct RemoteWatch {
  pub watcher: Pid,
  pub watchee: Pid,
}

#[derive(Debug, Clone, PartialEq, Message)]
pub struct RemoteUnwatch {
  pub watcher: Pid,
  pub watchee: Pid,
}

#[derive(Debug, Clone, PartialEq, Message)]
pub struct RemoteDeliver {
  pub header: Option<ReadonlyMessageHeadersHandle>,
  pub message: MessageHandle,
  pub target: Pid,
  pub sender: Option<Pid>,
  pub serializer_id: u32,
}

#[derive(Debug, Clone)]
pub struct JsonMessage {
  pub type_name: String,
  pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct Ping;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct Pong;

#[derive(Debug, Clone, PartialEq, Message)]
pub struct RemoteTerminate {
  pub watcher: Option<Pid>,
  pub watchee: Option<Pid>,
}

impl Eq for RemoteMessage {}

impl Hash for RemoteMessage {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.message_type.hash(state);
  }
}

impl Hash for ClientConnection {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.system_id.hash(state);
  }
}

impl Hash for ServerConnection {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.system_id.hash(state);
    self.address.hash(state);
  }
}

impl Hash for ConnectionType {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      ConnectionType::ServerConnection(sc) => sc.hash(state),
      ConnectionType::ClientConnection(cc) => cc.hash(state),
    }
  }
}
fn hash_map_hash<K: Hash + Ord, V: Hash>(map: &HashMap<K, V>) -> u64 {
  let mut hasher = DefaultHasher::new();
  let mut entries: Vec<_> = map.iter().collect();
  entries.sort_by(|a, b| a.0.cmp(b.0));
  for (k, v) in entries {
    k.hash(&mut hasher);
    v.hash(&mut hasher);
  }
  hasher.finish()
}
impl Hash for MessageHeader {
  fn hash<H: Hasher>(&self, state: &mut H) {
    let v = hash_map_hash(&self.header_data);
    v.hash(state);
  }
}

impl Hash for MessageEnvelope {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.type_id.hash(state);
    self.message_data.hash(state);
    self.target.hash(state);
    self.sender.hash(state);
    self.serializer_id.hash(state);
    self.message_header.hash(state);
    self.target_request_id.hash(state);
    self.sender_request_id.hash(state);
  }
}

impl Hash for MessageBatch {
  fn hash<H: Hasher>(&self, state: &mut H) {
    for type_name in &self.type_names {
      type_name.hash(state);
    }
    for target in &self.targets {
      target.hash(state);
    }
    for envelope in &self.envelopes {
      envelope.hash(state);
    }
    for sender in &self.senders {
      sender.hash(state);
    }
  }
}

impl Hash for ConnectRequest {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.connection_type.hash(state);
  }
}

impl Hash for ConnectResponse {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.member_id.hash(state);
    self.blocked.hash(state);
  }
}

impl Hash for DisconnectRequest {
  fn hash<H: Hasher>(&self, _: &mut H) {}
}

impl Hash for MessageType {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      MessageType::MessageBatch(m) => m.hash(state),
      MessageType::ConnectRequest(m) => m.hash(state),
      MessageType::ConnectResponse(m) => m.hash(state),
      MessageType::DisconnectRequest(m) => m.hash(state),
    }
  }
}
