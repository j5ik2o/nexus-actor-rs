use crate::config::Config;
use crate::endpoint_manager::EndpointManager;
use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remote_message::MessageType;
use crate::generated::remote::remoting_client::RemotingClient;
use crate::generated::remote::{
  ConnectRequest, ConnectResponse, MessageBatch, MessageEnvelope, MessageHeader, RemoteMessage, ServerConnection,
};
use crate::messages::{EndpointConnectedEvent, EndpointEvent, EndpointTerminatedEvent, RemoteDeliver};
use crate::metrics::record_sender_snapshot;
use crate::remote::Remote;
use crate::serializer::RootSerializable;
use crate::serializer::{serialize_any, SerializerId};
use crate::{RemoteTransport, TransportEndpoint};
use async_trait::async_trait;
use dashmap::DashMap;
use futures::StreamExt;
use nexus_actor_core_rs::runtime::CoreTaskFuture;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid};
use nexus_actor_std_rs::actor::dispatch::DeadLetterEvent;
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_actor_std_rs::actor::metrics::metrics_impl::MetricsSink;
use nexus_actor_std_rs::generated::actor::{DeadLetterResponse, Pid};
use opentelemetry::KeyValue;
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Instant;
use thiserror::Error;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tonic::{Code, Response, Streaming};

#[derive(Debug, Clone)]
pub struct EndpointWriter {
  config: Config,
  address: String,
  conn: Arc<RwLock<Option<Channel>>>,
  stream: Arc<RwLock<Option<RemotingClient<Channel>>>>,
  transport_endpoint: TransportEndpoint,
  remote: Weak<Remote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointWriterError {
  #[error("Failed to connect to remote: {0}")]
  Connection(String),
  #[error("Failed to send connect request: {code}, {message}")]
  Response { code: Code, message: String },
  #[error("No response")]
  NoResponse,
  #[error("No field")]
  NoField,
}

impl EndpointWriter {
  pub fn new(remote: Weak<Remote>, address: String, config: Config, transport_endpoint: TransportEndpoint) -> Self {
    Self {
      config,
      address,
      conn: Arc::new(RwLock::new(None)),
      stream: Arc::new(RwLock::new(None)),
      remote,
      transport_endpoint,
    }
  }

  pub async fn take_conn(&mut self) -> Option<Channel> {
    let mut mg = self.conn.write().await;
    mg.take()
  }

  pub async fn get_conn(&self) -> Option<Channel> {
    let mg = self.conn.read().await;
    mg.clone()
  }

  async fn set_conn(&mut self, conn: Channel) {
    let mut mg = self.conn.write().await;
    *mg = Some(conn);
  }

  async fn get_stream(&self) -> Option<RemotingClient<Channel>> {
    let mg = self.stream.read().await;
    mg.clone()
  }

  async fn take_stream(&self) -> Option<RemotingClient<Channel>> {
    let mut mg = self.stream.write().await;
    mg.take()
  }

  async fn set_stream(&self, stream: RemotingClient<Channel>) {
    let mut mg = self.stream.write().await;
    *mg = Some(stream);
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_actor_system()
      .clone()
  }

  pub async fn initialize(&mut self, _: ContextHandle) {
    let now = Instant::now();
    tracing::info!(address = %self.address, "Starting EndpointWriter connect loop");
    let _i = 0;
    let mut error: Option<EndpointWriterError> = None;
    for i in 0..self.config.get_max_retry_count().await {
      match self.initialize_internal().await {
        Ok(_) => {
          break;
        }
        Err(e) => {
          error = Some(e.clone());
          tracing::error!(
            "Failed to connect to remote: address = {}, retrying... {}",
            self.address,
            i
          );
          tokio::time::sleep(self.config.get_retry_interval().await).await;
          continue;
        }
      }
    }

    if error.is_some() {
      let terminated = EndpointEvent::EndpointTerminated(EndpointTerminatedEvent {
        address: self.address.clone(),
      });
      self
        .get_actor_system()
        .await
        .get_event_stream()
        .await
        .publish(MessageHandle::new(terminated))
        .await;
      return;
    }

    tracing::info!(
      address = %self.address,
      duration_ms = now.elapsed().as_millis(),
      "EndpointWriter connected"
    );
  }

  async fn create_channel(&self) -> Result<Channel, EndpointWriterError> {
    let remote = self
      .remote
      .upgrade()
      .ok_or_else(|| EndpointWriterError::Connection("remote dropped".to_string()))?;
    let transport = remote.transport();
    let handle = transport
      .connect(&self.transport_endpoint)
      .await
      .map_err(|_| EndpointWriterError::Connection("remote transport connect failed".to_string()))?;
    Ok(handle.channel())
  }

  async fn connect_request(
    &self,
    remote_client: &mut RemotingClient<Channel>,
  ) -> Result<Response<Streaming<RemoteMessage>>, EndpointWriterError> {
    let request = RemoteMessage {
      message_type: Some(MessageType::ConnectRequest(ConnectRequest {
        connection_type: Some(ConnectionType::ServerConnection(ServerConnection {
          system_id: self.get_actor_system().await.get_id().await,
          address: self.get_actor_system().await.get_address().await,
        })),
      })),
    };
    let request = tonic::Request::new(futures::stream::once(futures::future::ready(request)));
    remote_client
      .receive(request)
      .await
      .map_err(|status| EndpointWriterError::Response {
        code: status.code(),
        message: status.message().to_string(),
      })
  }

  async fn get_remote_message_in_response(
    response: &mut Response<Streaming<RemoteMessage>>,
  ) -> Result<RemoteMessage, EndpointWriterError> {
    let streaming = response.get_mut();

    let result = streaming.next().await;

    match result {
      None => Err(EndpointWriterError::NoResponse),
      Some(Err(e)) => Err(EndpointWriterError::Response {
        code: e.code(),
        message: e.message().to_string(),
      }),
      Some(Ok(remote_msg)) => Ok(remote_msg),
    }
  }

  async fn initialize_internal(&mut self) -> Result<(), EndpointWriterError> {
    let cloned_self = self.clone();

    let channel = self.create_channel().await?;
    assert!(self.get_conn().await.is_none(), "Connection is already set");
    self.set_conn(channel.clone()).await;

    let mut remote_client = RemotingClient::new(channel.clone());
    assert!(self.get_stream().await.is_none(), "Stream is already set");
    self.set_stream(remote_client.clone()).await;

    let mut streaming_response = self.connect_request(&mut remote_client).await?;
    let remote_message = Self::get_remote_message_in_response(&mut streaming_response).await?;
    // FIXME
    let connect_response = Self::get_connect_response(remote_message)?;
    tracing::info!(
      "Connected to remote: address = {}, connect_response = {:?}",
      self.address,
      connect_response
    );

    let actor_system = self.get_actor_system().await;
    let spawner = actor_system.core_runtime().spawner();
    let future: CoreTaskFuture = Box::pin(async move {
      let mut cloned_self = cloned_self.clone();
      let mut streaming = streaming_response.into_inner();

      while let Some(result) = streaming.next().await {
        let should_disconnect = match result {
          Err(e) => {
            tracing::error!("EndpointWriter failed to receive message: {}", e);
            true
          }
          Ok(msg) => matches!(msg.message_type, Some(MessageType::DisconnectRequest(_))),
        };

        if should_disconnect {
          cloned_self.on_disconnect("stream closed").await;
          return;
        }
      }
    });

    if let Err(err) = spawner.spawn(future) {
      tracing::error!(error = ?err, address = %self.address, "EndpointWriter failed to spawn stream listener");
    }

    let connected = EndpointEvent::EndpointConnected(EndpointConnectedEvent {
      address: self.address.clone(),
    });
    self.publish_stream(MessageHandle::new(connected)).await;

    Ok(())
  }

  async fn publish_stream(&self, msg: MessageHandle) {
    self
      .get_actor_system()
      .await
      .get_event_stream()
      .await
      .publish(msg)
      .await;
  }

  async fn endpoint_manager(&self) -> Option<EndpointManager> {
    let remote = self.remote.upgrade()?;
    remote.get_endpoint_manager_opt().await
  }

  async fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    let remote = self.remote.upgrade()?;
    remote.metrics_sink().await
  }

  async fn mark_deliver_success(&self) {
    if let Some(manager) = self.endpoint_manager().await {
      manager.increment_deliver_success(&self.address).await;
    }
  }

  async fn mark_deliver_failure(&self) {
    if let Some(manager) = self.endpoint_manager().await {
      manager.increment_deliver_failure(&self.address).await;
    }
  }

  async fn on_disconnect(&mut self, reason: &str) {
    tracing::warn!(address = %self.address, reason, "EndpointWriter detected disconnect; scheduling reconnect");
    self.close_client_conn().await;
    self
      .publish_stream(MessageHandle::new(EndpointEvent::EndpointTerminated(
        EndpointTerminatedEvent {
          address: self.address.clone(),
        },
      )))
      .await;
    if let Some(manager) = self.endpoint_manager().await {
      manager.schedule_reconnect(self.address.clone()).await;
    }
  }

  fn get_connect_response(remote_message: RemoteMessage) -> Result<ConnectResponse, EndpointWriterError> {
    match &remote_message.message_type {
      None => Err(EndpointWriterError::NoField),
      Some(message_type) => match message_type {
        MessageType::ConnectResponse(response) => Ok(response.clone()),
        _ => {
          tracing::error!(
            "EndpointWriter failed to receive connect response: {:?}",
            remote_message
          );
          Err(EndpointWriterError::Connection("Unknown message type".to_string()))
        }
      },
    }
  }

  async fn send_envelopes(
    &mut self,
    msg_list: impl IntoIterator<Item = MessageHandle>,
    ctx: &mut ContextHandle,
  ) -> Result<(), ActorError> {
    tracing::debug!("EndpointWriter send_envelopes");
    let mut envelopes = vec![];
    let metrics_sink = self.metrics_sink().await;

    let mut type_names = DashMap::new();
    let mut type_names_arr = vec![];

    let mut target_names = DashMap::new();
    let mut target_names_arr = vec![];

    let mut sender_names = DashMap::new();
    let mut sender_names_arr = vec![];

    let serializer_id = SerializerId::None;

    for msg in msg_list {
      let typed_msg = msg.to_typed::<EndpointEvent>();
      if let Some(EndpointEvent::EndpointTerminated(_)) = typed_msg {
        tracing::debug!("EndpointWriter received EndpointTerminated");
        ctx.stop(&ctx.get_self().await).await;
        return Ok(());
      }

      let rd = msg
        .to_typed::<RemoteDeliver>()
        .expect("Failed to convert to RemoteDeliver");

      tracing::trace!(deliver = ?rd, "EndpointWriter received deliver");

      if self.get_stream().await.is_none() {
        self.mark_deliver_failure().await;
        let actor_system = self.get_actor_system().await;
        if let Some(sender) = rd.sender {
          tracing::debug!("EndpointWriter sending DeadLetterResponse");
          let sender = ExtendedPid::new(sender);
          actor_system
            .get_root_context()
            .await
            .send(
              sender,
              MessageHandle::new(DeadLetterResponse {
                target: Some(rd.target.clone()),
              }),
            )
            .await;
        } else {
          tracing::debug!("EndpointWriter sending DeadLetterEvent");
          let target = ExtendedPid::new(rd.target);
          self
            .publish_stream(MessageHandle::new(DeadLetterEvent {
              message_handle: MessageHandle::new(rd.message.clone()),
              pid: Some(target),
              sender: None,
            }))
            .await;
        }
        continue;
      }

      let header = if rd.header.as_ref().map_or(true, |h| h.length() == 0) {
        None
      } else {
        let header_map: HashMap<String, String> = rd.header.unwrap().to_map().into_iter().collect();
        Some(MessageHeader {
          header_data: header_map,
        })
      };

      let message = rd.message;
      let message_type = message.get_type_name();

      tracing::trace!(message = ?message, "EndpointWriter processing message");

      let s_id = u32::from(serializer_id.clone());
      tracing::trace!(serializer_id = s_id, "EndpointWriter serializer");

      let request_opt = message.to_typed::<Arc<dyn RootSerializable>>();
      let v = request_opt.map(|request| request.serialize());

      // let deliver_batch_request_opt = message.to_typed::<DeliverBatchRequest>();
      // let pub_sub_auto_respond_batch_opt = message.to_typed::<PubSubAutoResponseBatch>();
      // let pub_sub_batch_opt = message.to_typed::<PubSubBatch>();
      //
      // let v = match (
      //   deliver_batch_request_opt,
      //   pub_sub_auto_respond_batch_opt,
      //   pub_sub_batch_opt,
      // ) {
      //   (Some(deliver_batch_request), _, _) => Some(deliver_batch_request.serialize()),
      //   (_, Some(pub_sub_auto_respond_batch), _) => Some(pub_sub_auto_respond_batch.serialize()),
      //   (_, _, Some(pub_sub_batch)) => Some(pub_sub_batch.serialize()),
      //   _ => None,
      // };

      if let Some(Err(e)) = &v {
        tracing::error!("Failed to serialize message: {:?}", e);
        continue;
      }

      let result = if let Some(Ok(msg)) = v {
        tracing::trace!("EndpointWriter serialize RootSerializable message");
        let result = serialize_any(msg.as_any(), &serializer_id, &msg.get_type_name());
        if let Err(e) = &result {
          tracing::error!("Failed to serialize message: {:?}", e);
          continue;
        }
        Some(result)
      } else {
        tracing::trace!("EndpointWriter serialize message via serialize_any");
        Some(serialize_any(
          message.as_any(),
          &serializer_id,
          &message.get_type_name(),
        ))
      };

      tracing::trace!(result = ?result, "EndpointWriter serialized message");

      let bytes = result.expect("Not found message").expect("Failed to serialize message");

      if let Some(sink) = metrics_sink.as_ref() {
        let labels = vec![
          KeyValue::new("remote.endpoint", self.address.clone()),
          KeyValue::new("remote.direction", "outbound".to_string()),
          KeyValue::new("remote.message_type", message_type.clone()),
        ];
        sink.record_message_size_with_labels(bytes.len() as u64, &labels);
      }

      tracing::trace!("EndpointWriter obtained serialized bytes");

      let type_id = add_to_lookup(&mut type_names, message_type.clone(), &mut type_names_arr);
      let target_id = add_to_target_lookup(&mut target_names, &rd.target, &mut target_names_arr);
      let target_request_id = rd.target.request_id;

      let sender_id = add_to_sender_lookup(&mut sender_names, rd.sender.as_ref(), &mut sender_names_arr);
      let sender_request_id = if rd.sender.is_none() {
        0
      } else {
        rd.sender.as_ref().unwrap().request_id
      };

      let me = MessageEnvelope {
        type_id,
        message_data: bytes,
        target: target_id,
        sender: sender_id,
        serializer_id: s_id,
        message_header: header,
        target_request_id,
        sender_request_id,
      };

      tracing::trace!(envelope = ?me, "EndpointWriter built envelope");

      envelopes.push(me);
    }

    if envelopes.is_empty() {
      tracing::debug!("EndpointWriter had no envelopes to send");
      return Ok(());
    }

    let mut stream = self.get_stream().await.expect("Stream is not set");

    tracing::trace!(envelopes = ?envelopes, "EndpointWriter batch envelopes");

    let request = RemoteMessage {
      message_type: Some(MessageType::MessageBatch(MessageBatch {
        type_names: type_names_arr,
        targets: target_names_arr,
        envelopes,
        senders: sender_names_arr,
      })),
    };

    let request = tonic::Request::new(futures::stream::once(futures::future::ready(request)));
    tracing::trace!("EndpointWriter sending message batch");
    let response = stream.receive(request).await;

    if let Err(e) = &response {
      tracing::error!("Failed to send message: {:?}", e);
      self.mark_deliver_failure().await;
      self.on_disconnect("send failure").await;
      ctx.stash().await;
      ctx.stop(&ctx.get_self().await).await;
      return Ok(());
    }

    self.mark_deliver_success().await;
    response
      .map(|_| ())
      .map_err(|e| ActorError::ReceiveError(ErrorReason::from(e.to_string())))?;
    Ok(())
  }

  async fn close_client_conn(&mut self) {
    if self.get_stream().await.is_some() {
      let Some(s) = self.take_stream().await else {
        panic!("Stream is already taken")
      };
      drop(s);
    }
    if self.get_conn().await.is_some() {
      let Some(c) = self.take_conn().await else {
        panic!("Connection is already taken")
      };
      drop(c);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Weak;

  #[tokio::test]
  async fn create_channel_fails_when_remote_dropped() {
    let config = Config::default();
    let mut writer = EndpointWriter::new(
      Weak::new(),
      "127.0.0.1:5000".to_string(),
      config,
      TransportEndpoint::new("127.0.0.1:5000".to_string()),
    );

    let err = writer.create_channel().await.expect_err("remote upgrade should fail");

    assert!(matches!(err, EndpointWriterError::Connection(msg) if msg == "remote dropped"));
  }
}

fn add_to_lookup(m: &mut DashMap<String, i32>, name: String, a: &mut Vec<String>) -> i32 {
  let max = m.len() as i32;

  let id = match m.get(&name) {
    Some(id) => *id,
    None => {
      m.insert(name.clone(), max);
      a.push(name);
      max
    }
  };

  id
}

fn add_to_target_lookup(m: &mut DashMap<String, i32>, pid: &Pid, arr: &mut Vec<Pid>) -> i32 {
  let max = m.len() as i32;
  let key = format!("{}/{}", pid.address, pid.id);

  let id = match m.get(&key) {
    Some(id) => *id,
    None => {
      let mut c = pid.clone();
      c.request_id = 0;
      m.insert(key, max);
      arr.push(c);
      max
    }
  };

  id
}

fn add_to_sender_lookup(m: &mut DashMap<String, i32>, pid: Option<&Pid>, arr: &mut Vec<Pid>) -> i32 {
  let pid = match pid {
    Some(p) => p,
    None => return 0,
  };

  let max = m.len() as i32;
  tracing::trace!(max = max, "add_to_sender_lookup");
  let key = format!("{}/{}", pid.address, pid.id);
  let id_ref = m.get(&key);
  let mut id = id_ref.map(|id| *id);
  match id {
    Some(id) => id,
    None => {
      let mut c = pid.clone();
      c.request_id = 0;
      m.insert(key, max);
      id = Some(max);
      arr.push(c);
      max
    }
  };
  tracing::trace!(id = ?id, "add_to_sender_lookup id");
  id.expect("not found value") + 1
}

#[async_trait]
impl Actor for EndpointWriter {
  async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("EndpointWriter received message");
    let _ = record_sender_snapshot(&context_handle).await;
    let msg = if let Some(handle) = context_handle.try_get_message_handle_opt() {
      handle
    } else {
      context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
    };
    let endpoint_event = msg.to_typed::<EndpointEvent>();
    match endpoint_event {
      Some(EndpointEvent::EndpointTerminated(_)) => {
        context_handle.stop(&context_handle.get_self().await).await;
      }
      _ => {
        let _ = self.send_envelopes(vec![msg], &mut context_handle).await?;
      }
    }
    Ok(())
  }

  async fn post_start(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    self.initialize(ctx).await;
    Ok(())
  }

  async fn pre_restart(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    self.close_client_conn().await;
    Ok(())
  }

  async fn pre_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    self.close_client_conn().await;
    Ok(())
  }
}
