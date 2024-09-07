use crate::actor::message::Message;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::generated::actor::Pid;
use crate::Message;

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

#[derive(Debug, Clone)]
pub struct RemoteWatch {
  pub watcher: Pid,
  pub watchee: Pid,
}

#[derive(Debug, Clone)]
pub struct RemoteUnwatch {
  pub watcher: Pid,
  pub watchee: Pid,
}

#[derive(Debug, Clone)]
pub struct RemoteDeliver {
  pub header: ReadonlyMessageHeadersHandle,
  pub message: Vec<u8>,
  pub target: Pid,
  pub sender: Pid,
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
