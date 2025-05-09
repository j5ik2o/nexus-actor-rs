use crate::generated::cluster::{
  DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport, Subscribers,
};
use crate::serializer::{RootSerializable, RootSerialized, SerializerError};
use nexus_actor_core_rs::actor::message::Message;
use nexus_actor_message_derive_rs::Message;
use std::sync::Arc;

#[derive(Debug, Clone, Message)]
pub struct PubSubBatch {
  envelopes: Vec<Arc<dyn Message>>,
}

impl PartialEq for PubSubBatch {
  fn eq(&self, other: &Self) -> bool {
    self
      .envelopes
      .iter()
      .all(|e| other.envelopes.iter().any(|o| e.eq_message(&*o.clone())))
  }
}

impl RootSerializable for PubSubBatch {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    todo!()
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
pub struct DeliverBatchRequest {
  subscribers: Subscribers,
  pub_sub_batch: PubSubBatch,
  topic: String,
}

impl RootSerializable for DeliverBatchRequest {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    todo!()
  }
}

#[derive(Debug, Clone, Message)]
pub struct PubSubAutoResponseBatch {
  envelopes: Vec<Arc<dyn Message>>,
}

impl PartialEq for PubSubAutoResponseBatch {
  fn eq(&self, other: &Self) -> bool {
    self
      .envelopes
      .iter()
      .all(|e| other.envelopes.iter().any(|o| e.eq_message(&*o.clone())))
  }
}

impl RootSerializable for PubSubAutoResponseBatch {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    todo!()
  }
}

impl RootSerialized for PubSubBatchTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    todo!()
  }
}

impl RootSerialized for DeliverBatchRequestTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    todo!()
  }
}

impl RootSerialized for PubSubAutoRespondBatchTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    todo!()
  }
}
