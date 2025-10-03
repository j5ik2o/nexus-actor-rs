use crate::generated::cluster::{
  DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport, Subscribers,
};
use crate::serializer::{
  deserialize_message, serialize_any, RootSerializable, RootSerialized, SerializerError, SerializerId,
};
use nexus_actor_std_rs::actor::message::Message;
use nexus_message_derive_rs::Message;
use std::collections::HashMap;
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
    let mut transport = PubSubBatchTransport {
      type_names: Vec::new(),
      envelopes: Vec::new(),
    };

    let mut type_index_map = HashMap::new();

    for envelope in &self.envelopes {
      let type_name = envelope.get_type_name();

      let entry = type_index_map.entry(type_name.clone()).or_insert_with(|| {
        transport.type_names.push(type_name.clone());
        (transport.type_names.len() - 1) as i32
      });

      let serializer_id = SerializerId::None;
      let data = serialize_any(envelope.as_any(), &serializer_id, &type_name)?;

      transport.envelopes.push(crate::generated::cluster::PubSubEnvelope {
        type_id: *entry,
        message_data: data,
        serializer_id: u32::from(serializer_id) as i32,
      });
    }

    Ok(Arc::new(transport))
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
pub struct DeliverBatchRequest {
  subscribers: Option<Subscribers>,
  pub_sub_batch: PubSubBatch,
  topic: String,
}

impl RootSerializable for DeliverBatchRequest {
  fn serialize(&self) -> Result<Arc<dyn RootSerialized>, SerializerError> {
    let batch_transport = self.pub_sub_batch.serialize()?;

    let batch_transport = batch_transport
      .as_any()
      .downcast_ref::<PubSubBatchTransport>()
      .ok_or_else(|| SerializerError::serialization("Failed to downcast to PubSubBatchTransport"))?
      .clone();

    Ok(Arc::new(DeliverBatchRequestTransport {
      subscribers: self.subscribers.clone(),
      batch: Some(batch_transport),
      topic: self.topic.clone(),
    }))
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
    let batch_transport = PubSubBatch {
      envelopes: self.envelopes.clone(),
    }
    .serialize()?;

    let batch_transport = batch_transport
      .as_any()
      .downcast_ref::<PubSubBatchTransport>()
      .ok_or_else(|| SerializerError::serialization("Failed to downcast to PubSubBatchTransport"))?
      .clone();

    Ok(Arc::new(PubSubAutoRespondBatchTransport {
      type_names: batch_transport.type_names,
      envelopes: batch_transport.envelopes,
    }))
  }
}

impl RootSerialized for PubSubBatchTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    let mut batch = PubSubBatch {
      envelopes: Vec::with_capacity(self.envelopes.len()),
    };

    for envelope in &self.envelopes {
      let type_name = self
        .type_names
        .get(envelope.type_id as usize)
        .ok_or_else(|| SerializerError::deserialization("Invalid type id"))?
        .clone();

      let serializer_raw = u32::try_from(envelope.serializer_id)
        .map_err(|_| SerializerError::deserialization("Negative serializer id"))?;

      let serializer_id = SerializerId::try_from(serializer_raw).map_err(SerializerError::deserialization)?;

      let message = deserialize_message(&envelope.message_data, &serializer_id, &type_name)?;
      batch.envelopes.push(message);
    }

    Ok(Arc::new(batch))
  }
}

impl RootSerialized for DeliverBatchRequestTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    let batch = self
      .batch
      .as_ref()
      .ok_or_else(|| SerializerError::deserialization("Batch is missing"))?
      .deserialize()?;

    let pub_sub_batch = batch
      .as_any()
      .downcast_ref::<PubSubBatch>()
      .ok_or_else(|| SerializerError::deserialization("Failed to downcast to PubSubBatch"))?
      .clone();

    Ok(Arc::new(DeliverBatchRequest {
      subscribers: self.subscribers.clone(),
      pub_sub_batch,
      topic: self.topic.clone(),
    }))
  }
}

impl RootSerialized for PubSubAutoRespondBatchTransport {
  fn deserialize(&self) -> Result<Arc<dyn RootSerializable>, SerializerError> {
    let batch_transport = PubSubBatchTransport {
      type_names: self.type_names.clone(),
      envelopes: self.envelopes.clone(),
    };

    let batch = batch_transport.deserialize()?;

    let pub_sub_batch = batch
      .as_any()
      .downcast_ref::<PubSubBatch>()
      .ok_or_else(|| SerializerError::deserialization("Failed to downcast to PubSubBatch"))?
      .clone();

    Ok(Arc::new(PubSubAutoResponseBatch {
      envelopes: pub_sub_batch.envelopes,
    }))
  }
}
