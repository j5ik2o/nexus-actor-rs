use crate::actor::actor::{Actor, ActorError, ErrorReason, ExtendedPid};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart, StopperPart};
use crate::actor::dispatch::DeadLetterEvent;
use crate::actor::message::{Message, MessageHandle, ReadonlyMessageHeaders};
use crate::cluster::messages::{DeliverBatchRequest, PubSubAutoResponseBatch, PubSubBatch};
use crate::generated::actor::{DeadLetterResponse, Pid};
use crate::generated::cluster::{DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport};
use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remote_message::MessageType;
use crate::generated::remote::remoting_client::RemotingClient;
use crate::generated::remote::{
  ConnectRequest, ConnectResponse, MessageBatch, MessageEnvelope, MessageHeader, RemoteMessage, ServerConnection,
};
use crate::remote::config::Config;
use crate::remote::messages::{EndpointConnectedEvent, EndpointEvent, EndpointTerminatedEvent, RemoteDeliver};
use crate::remote::remote::Remote;
use crate::remote::serializer::{serialize, serialize_any, RootSerializable, SerializerId};
use async_trait::async_trait;
use dashmap::DashMap;
use futures::{StreamExt, TryFutureExt};
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
  remote: Weak<Remote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointWriterError {
  #[error("Invalid URL: {0}")]
  InvalidUrlError(String),
  #[error("Failed to connect to remote: {0}")]
  ConnectionError(String),
  #[error("Failed to send connect request: {code}, {message}")]
  ResponseError { code: Code, message: String },
  #[error("No response")]
  NoResponseError,
  #[error("No field")]
  NoFieldError,
}

impl EndpointWriter {
  pub fn new(remote: Weak<Remote>, address: String, config: Config) -> Self {
    Self {
      config,
      address,
      conn: Arc::new(RwLock::new(None)),
      stream: Arc::new(RwLock::new(None)),
      remote,
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
    tracing::info!("Started EndpointWriter. connecting: to {}", self.address);
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
      "EndpointWriter connected to remote: address = {}, cost = {} millis",
      self.address,
      now.elapsed().as_millis()
    );
  }

  async fn create_channel(&self) -> Result<Channel, EndpointWriterError> {
    let address = format!("http://{}", self.address);
    let endpoint = Channel::from_shared(address).map_err(|e| EndpointWriterError::InvalidUrlError(e.to_string()))?;
    endpoint
      .connect()
      .map_err(|e| EndpointWriterError::ConnectionError(e.to_string()))
      .await
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
      .map_err(|status| EndpointWriterError::ResponseError {
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
      None => Err(EndpointWriterError::NoResponseError),
      Some(Err(e)) => Err(EndpointWriterError::ResponseError {
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

    tokio::spawn(async move {
      let mut cloned_self = cloned_self.clone();
      let mut streaming = streaming_response.into_inner();
      while let Some(result) = streaming.next().await {
        match &result {
          Err(e) => {
            tracing::error!("EndpointWriter failed to receive message: {}", e);
            let terminated = EndpointEvent::EndpointTerminated(EndpointTerminatedEvent {
              address: cloned_self.address.clone(),
            });
            cloned_self.publish_stream(MessageHandle::new(terminated)).await;
            return;
          }
          Ok(msg) => match &msg.message_type {
            None => {
              return;
            }
            Some(message_type) => match message_type {
              MessageType::DisconnectRequest(_) => {
                let terminated = EndpointEvent::EndpointTerminated(EndpointTerminatedEvent {
                  address: cloned_self.address.clone(),
                });
                cloned_self.publish_stream(MessageHandle::new(terminated)).await;
              }
              _ => {
                return;
              }
            },
          },
        }
      }
    });

    let connected = EndpointEvent::EndpointConnected(EndpointConnectedEvent {
      address: self.address.clone(),
    });
    self.publish_stream(MessageHandle::new(connected)).await;

    Ok(())
  }

  async fn publish_stream(&mut self, msg: MessageHandle) {
    self
      .get_actor_system()
      .await
      .get_event_stream()
      .await
      .publish(msg)
      .await;
  }

  fn get_connect_response(remote_message: RemoteMessage) -> Result<ConnectResponse, EndpointWriterError> {
    match &remote_message.message_type {
      None => Err(EndpointWriterError::NoFieldError),
      Some(message_type) => match message_type {
        MessageType::ConnectResponse(response) => Ok(response.clone()),
        _ => {
          tracing::error!(
            "EndpointWriter failed to receive connect response: {:?}",
            remote_message
          );
          Err(EndpointWriterError::ConnectionError("Unknown message type".to_string()))
        }
      },
    }
  }

  async fn send_envelopes(
    &mut self,
    msg_list: impl IntoIterator<Item = MessageHandle>,
    ctx: &mut ContextHandle,
  ) -> Result<(), ActorError> {
    tracing::info!("EndpointWriter send_envelopes");
    let mut envelopes = vec![];

    let mut type_names = DashMap::new();
    let mut type_names_arr = vec![];

    let mut target_names = DashMap::new();
    let mut target_names_arr = vec![];

    let mut sender_names = DashMap::new();
    let mut sender_names_arr = vec![];

    let serializer_id = SerializerId::None;

    for msg in msg_list {
      let typed_msg = msg.to_typed::<EndpointEvent>();
      match typed_msg {
        Some(EndpointEvent::EndpointTerminated(_)) => {
          tracing::info!("EndpointWriter received EndpointTerminated");
          ctx.stop(&ctx.get_self().await).await;
          return Ok(());
        }
        _ => {}
      }

      let rd = msg
        .to_typed::<RemoteDeliver>()
        .expect("Failed to convert to RemoteDeliver");

      tracing::info!("EndpointWriter: get {:?}", rd);

      let header;
      let message;

      if self.get_stream().await.is_none() {
        let actor_system = self.get_actor_system().await;
        if let Some(sender) = rd.sender {
          tracing::info!("EndpointWriter sending DeadLetterResponse");
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
          tracing::info!("EndpointWriter sending DeadLetterEvent");
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

      header = if rd.header.as_ref().map_or(true, |h| h.length() == 0) {
        None
      } else {
        Some(MessageHeader {
          header_data: rd.header.unwrap().to_map(),
        })
      };

      message = rd.message;

      tracing::info!("message = {:?}", message);

      let s_id = u32::try_from(serializer_id.clone()).unwrap();
      tracing::info!("EndpointWriter: serializer_id = {:?}", s_id);

      let deliver_batch_request_opt = message.to_typed::<DeliverBatchRequest>();
      let pub_sub_auto_respond_batch_opt = message.to_typed::<PubSubAutoResponseBatch>();
      let pub_sub_batch_opt = message.to_typed::<PubSubBatch>();

      let v = match (
        deliver_batch_request_opt,
        pub_sub_auto_respond_batch_opt,
        pub_sub_batch_opt,
      ) {
        (Some(deliver_batch_request), _, _) => Some(deliver_batch_request.serialize()),
        (_, Some(pub_sub_auto_respond_batch), _) => Some(pub_sub_auto_respond_batch.serialize()),
        (_, _, Some(pub_sub_batch)) => Some(pub_sub_batch.serialize()),
        _ => None,
      };

      if let Some(Err(e)) = v {
        tracing::error!("Failed to serialize message: {:?}", e);
        continue;
      }

      let result = if let Some(Ok(msg)) = v {
        tracing::info!("EndpointWriter: serialize message");
        let deliver_batch_request_transport_opt = msg.as_any().downcast_ref::<DeliverBatchRequestTransport>().cloned();
        let pub_sub_auto_respond_batch_transport_opt =
          msg.as_any().downcast_ref::<PubSubAutoRespondBatchTransport>().cloned();
        let pub_sub_batch_transport_opt = msg.as_any().downcast_ref::<PubSubBatchTransport>().cloned();

        let result = match (
          deliver_batch_request_transport_opt,
          pub_sub_auto_respond_batch_transport_opt,
          pub_sub_batch_transport_opt,
        ) {
          (Some(deliver_batch_request_transport), _, _) => {
            Some(serialize(&deliver_batch_request_transport, &serializer_id))
          }
          (_, Some(pub_sub_auto_respond_batch_transport), _) => {
            Some(serialize(&pub_sub_auto_respond_batch_transport, &serializer_id))
          }
          (_, _, Some(pub_sub_batch_transport)) => Some(serialize(&pub_sub_batch_transport, &serializer_id)),
          _ => None,
        };
        if let Some(Err(e)) = result {
          tracing::error!("Failed to serialize message: {:?}", e);
          continue;
        }
        result
      } else {
        tracing::info!("EndpointWriter: serialize_any message");
        Some(serialize_any(
          message.as_any(),
          &serializer_id,
          &message.get_type_name(),
        ))
      };

      tracing::info!("EndpointWriter: serialized message: {:?}", result);

      let bytes = result.expect("Not found message").expect("Failed to serialize message");

      tracing::info!("EndpointWriter: get bytes");

      let type_id = add_to_lookup(&mut type_names, message.get_type_name(), &mut type_names_arr);
      let target_id = add_to_target_lookup(&mut target_names, &rd.target, &mut target_names_arr);
      let target_request_id = rd.target.request_id.clone();

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

      tracing::info!("EndpointWriter: message envelope = {:?}", me);

      envelopes.push(me);
    }

    if envelopes.is_empty() {
      tracing::info!("EndpointWriter envelopes is empty");
      return Ok(());
    }

    let mut stream = self.get_stream().await.expect("Stream is not set");

    tracing::info!("EndpointWriter: envelopes = {:?}", envelopes);

    let request = RemoteMessage {
      message_type: Some(MessageType::MessageBatch(MessageBatch {
        type_names: type_names_arr,
        targets: target_names_arr,
        envelopes,
        senders: sender_names_arr,
      })),
    };

    let request = tonic::Request::new(futures::stream::once(futures::future::ready(request)));
    tracing::info!("EndpointWriter sending message batch: {:?}", request);
    let response = stream.receive(request).await;

    if let Err(e) = &response {
      ctx.stash().await;
      tracing::error!("Failed to send message: {:?}", e);
      ctx.stop(&ctx.get_self().await).await;
    }

    let _ = response
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
  tracing::info!("add_to_sender_lookup: max = {}", max);
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
  tracing::info!("add_to_sender_lookup: id = {:?}", id);
  id.expect("not found value") + 1
}

#[async_trait]
impl Actor for EndpointWriter {
  async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("EndpointWriter received message");
    let msg = context_handle.get_message_handle().await;
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
