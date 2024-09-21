use crate::generated::cluster::{
  DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport, Subscribers,
};
use crate::remote::serializer::{RootSerializable, RootSerialized, SerializerError};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PubSubBatch {
  envelopes: Vec<Arc<dyn prost::Message>>,
}

impl RootSerializable for PubSubBatch {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    todo!()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[derive(Debug, Clone)]
pub struct DeliverBatchRequest {
  subscribers: Subscribers,
  pub_sub_batch: PubSubBatch,
  topic: String,
}

impl RootSerializable for DeliverBatchRequest {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    todo!()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[derive(Debug, Clone)]
pub struct PubSubAutoResponseBatch {
  envelopes: Vec<Arc<dyn prost::Message>>,
}

impl RootSerializable for PubSubAutoResponseBatch {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    todo!()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl RootSerialized for PubSubBatchTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    todo!()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl RootSerialized for DeliverBatchRequestTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    todo!()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl RootSerialized for PubSubAutoRespondBatchTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    todo!()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
