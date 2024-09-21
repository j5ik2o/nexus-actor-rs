use crate::actor::actor::{Actor, ExtendedPid};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::SenderPart;
use crate::actor::message::{Message, MessageEnvelope, MessageHandle, MessageHeaders, SystemMessage};
use crate::actor::process::Process;
use crate::generated::actor::{Pid, Stop, Terminated, Unwatch, Watch};
use crate::generated::cluster::{DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport};
use crate::generated::remote;
use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remoting_server::Remoting;
use crate::generated::remote::{
  ConnectRequest, GetProcessDiagnosticsRequest, GetProcessDiagnosticsResponse, ListProcessesRequest,
  ListProcessesResponse, MessageBatch, RemoteMessage, ServerConnection,
};
use crate::remote::endpoint_manager::{EndpointManager, RequestKeyWrapper};
use crate::remote::messages::RemoteTerminate;
use crate::remote::remote::Remote;
use crate::remote::serializer::{
  deserialize, deserialize_any, deserialize_message, AnyDowncastExt, RootSerialized, SerializerId,
};
use std::any::Any;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};
use tonic::codegen::tokio_stream::wrappers::ReceiverStream;
use tonic::codegen::tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

macro_rules! try_deserialize_all {
    ($data:expr, $serializer_id:expr, $($t:ty),+) => {
        (
            $(
                deserialize::<$t>($data, $serializer_id).ok(),
            )+
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointReaderError {
  #[error("Unknown target")]
  UnknownTargetError,
  #[error("Unknown sender")]
  UnknownSenderError,
  #[error("Deserialization error: {0}")]
  DeserializationError(String),
}

#[derive(Debug, Clone)]
pub(crate) struct EndpointReader {
  suspended: Arc<AtomicBool>,
  remote: Weak<Remote>,
}

impl EndpointReader {
  pub(crate) fn new(remote: Weak<Remote>) -> Self {
    EndpointReader {
      suspended: Arc::new(AtomicBool::new(false)),
      remote,
    }
  }

  async fn on_connect_request(
    &self,
    response_tx: &Sender<Result<RemoteMessage, Status>>,
    connect_req: &ConnectRequest,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    match &connect_req.connection_type {
      Some(ConnectionType::ServerConnection(sc)) => {
        self.on_server_connection(response_tx, sc).await;
      }
      Some(ConnectionType::ClientConnection(_)) => {
        // TODO: implement me
        tracing::error!("ClientConnection not implemented");
      }
      _ => {
        tracing::warn!("Received unknown connection type");
        return Ok(true);
      }
    }
    Ok(false)
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_actor_system()
      .clone()
  }

  async fn on_server_connection(&self, response_tx: &Sender<Result<RemoteMessage, Status>>, sc: &ServerConnection) {
    if self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_block_list()
      .is_blocked(&sc.system_id)
      .await
    {
      tracing::debug!("EndpointReader blocked connection from {}", sc.system_id);
      if let Err(e) = response_tx
        .send(Ok(RemoteMessage {
          message_type: Some(remote::remote_message::MessageType::ConnectResponse(
            remote::ConnectResponse {
              blocked: true,
              member_id: sc.system_id.clone(),
            },
          )),
        }))
        .await
      {
        tracing::error!("EndpointReader failed to send ConnectResponse message: {}", e);
      }
    } else {
      tracing::debug!("EndpointReader accepted connection from {}", sc.system_id);
      if let Err(e) = response_tx
        .send(Ok(RemoteMessage {
          message_type: Some(remote::remote_message::MessageType::ConnectResponse(
            remote::ConnectResponse {
              blocked: false,
              member_id: sc.system_id.clone(),
            },
          )),
        }))
        .await
      {
        tracing::error!("EndpointReader failed to send ConnectResponse message: {}", e);
      }
    }
  }

  async fn on_message_batch(&self, message_batch: &MessageBatch) -> Result<(), EndpointReaderError> {
    tracing::info!("EndpointReader received message batch: {:?}", message_batch);
    for envelope in &message_batch.envelopes {
      let data = &envelope.message_data;
      let mut sender = Pid::default();
      let mut target = Pid::default();

      let sender_opt = deserialize_sender(
        &mut sender,
        envelope.sender,
        envelope.sender_request_id,
        &message_batch.senders,
      );

      tracing::info!("envelope.sender = {:?}", envelope.sender);
      tracing::info!("envelope.senders = {:?}", message_batch.senders);
      tracing::info!("sender_opt = {:?}", sender_opt);

      let target = deserialize_target(
        &mut target,
        envelope.target,
        envelope.target_request_id,
        &message_batch.targets,
      )
      .map(ExtendedPid::new)
      .ok_or_else(|| {
        tracing::error!("EndpointReader received message with unknown target");
        EndpointReaderError::UnknownTargetError
      })?;

      // TODO
      let serializer_id = SerializerId::try_from(envelope.serializer_id).expect("Invalid serializer id");

      let (pub_sub_batch_transport_opt, deliver_batch_request_transport_opt, pub_sub_auto_respond_batch_transport_opt) = try_deserialize_all!(
        &data,
        &serializer_id,
        PubSubBatchTransport,
        DeliverBatchRequestTransport,
        PubSubAutoRespondBatchTransport
      );

      let result = match (
        pub_sub_batch_transport_opt,
        deliver_batch_request_transport_opt,
        pub_sub_auto_respond_batch_transport_opt,
      ) {
        (Some(pub_sub_batch_transport), _, _) => Some(pub_sub_batch_transport.deserialize()),
        (_, Some(deliver_batch_request_transport), _) => Some(deliver_batch_request_transport.deserialize()),
        (_, _, Some(pub_sub_auto_respond_batch_transport)) => Some(pub_sub_auto_respond_batch_transport.deserialize()),
        _ => None,
      };

      match result {
        Some(message) => {
          let m = message.map_err(|e| EndpointReaderError::DeserializationError(e.to_string()))?;
          if let Some(t) = m.as_any().downcast_ref::<Terminated>() {
            let terminated = SystemMessage::of_terminate(t.clone());
            self
              .get_actor_system()
              .await
              .get_root_context()
              .await
              .send(target.clone(), MessageHandle::new(terminated))
              .await;
          }
          if let Some(stop) = m.as_any().downcast_ref::<Stop>() {
            let system_message = SystemMessage::of_stop();
            let ref_process = self
              .get_actor_system()
              .await
              .get_process_registry()
              .await
              .get_local_process(target.id())
              .await
              .ok_or(EndpointReaderError::UnknownTargetError)?;
            ref_process
              .send_system_message(&target, MessageHandle::new(system_message))
              .await;
          }
          if let Some(watch) = m.as_any().downcast_ref::<Watch>() {
            let system_message = SystemMessage::of_watch(watch.clone());
            let ref_process = self
              .get_actor_system()
              .await
              .get_process_registry()
              .await
              .get_local_process(target.id())
              .await
              .ok_or(EndpointReaderError::UnknownTargetError)?;
            ref_process
              .send_system_message(&target, MessageHandle::new(system_message))
              .await;
          }
          if let Some(unwatch) = m.as_any().downcast_ref::<Unwatch>() {
            let system_message = SystemMessage::of_unwatch(unwatch.clone());
            let ref_process = self
              .get_actor_system()
              .await
              .get_process_registry()
              .await
              .get_local_process(target.id())
              .await
              .ok_or(EndpointReaderError::UnknownTargetError)?;
            ref_process
              .send_system_message(&target, MessageHandle::new(system_message))
              .await;
          }
        }
        None => {
          let type_name = message_batch.type_names.get(envelope.type_id as usize).unwrap();
          let data_arc = deserialize_message(data, &serializer_id, type_name)
            .map_err(|e| EndpointReaderError::DeserializationError(e.to_string()))?;
          let msg_handle = MessageHandle::new_arc(data_arc.clone());
          tracing::info!("EndpointReader received message: {:?}", data_arc);

          if sender_opt.is_none() && envelope.message_header.is_none() {
            tracing::info!("EndpointReader received message with no sender and no header");
            self
              .get_actor_system()
              .await
              .get_root_context()
              .await
              .send(target, msg_handle)
              .await;
            continue;
          }

          let headers = if envelope.message_header.is_some() {
            MessageHeaders::with_values(envelope.message_header.as_ref().unwrap().header_data.clone())
          } else {
            MessageHeaders::default()
          };

          let sender = ExtendedPid::new(sender_opt.expect("Sender not found"));
          let local_me = MessageEnvelope::new(msg_handle)
            .with_header(headers)
            .with_sender(sender);
          tracing::info!("EndpointReader received message: {:?}", local_me);
          tracing::info!("EndpointReader: target: {:?}", target);
          self
            .get_actor_system()
            .await
            .get_root_context()
            .await
            .send(target, MessageHandle::new(local_me))
            .await;
        }
      }
    }
    Ok(())
  }

  pub fn set_suspend(&mut self, suspend: bool) {
    self.suspended.store(suspend, std::sync::atomic::Ordering::SeqCst);
  }

  async fn get_suspend(suspended: Arc<Mutex<bool>>) -> bool {
    *suspended.lock().await
  }

  async fn get_disconnect_flg(disconnect_rx: Arc<Mutex<Receiver<bool>>>) -> bool {
    let mut disconnect_mg = disconnect_rx.lock().await;
    if let Some(b) = disconnect_mg.recv().await {
      b
    } else {
      false
    }
  }

  async fn get_endpoint_manager_opt(&self) -> Option<EndpointManager> {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_endpoint_manager_opt()
      .await
  }

  async fn get_endpoint_manager(&self) -> EndpointManager {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_endpoint_manager()
      .await
  }
}

fn deserialize_sender<'a>(pid: &'a mut Pid, index: i32, request_id: u32, arr: &[Pid]) -> Option<Pid> {
  if index == 0 {
    None
  } else {
    *pid = arr[index as usize - 1].clone();
    if request_id > 0 {
      *pid = pid.clone();
      pid.request_id = request_id;
      Some(pid.clone())
    } else {
      Some(pid.clone())
    }
  }
}

fn deserialize_target<'a>(pid: &'a mut Pid, index: i32, request_id: u32, arr: &[Pid]) -> Option<Pid> {
  *pid = arr[index as usize].clone();
  if request_id > 0 {
    *pid = pid.clone();
    pid.request_id = request_id;
    Some(pid.clone())
  } else {
    Some(pid.clone())
  }
}

#[tonic::async_trait]
impl Remoting for EndpointReader {
  type ReceiveStream = Pin<Box<dyn Stream<Item = Result<RemoteMessage, Status>> + Send>>;

  async fn receive(&self, request: Request<Streaming<RemoteMessage>>) -> Result<Response<Self::ReceiveStream>, Status> {
    tracing::info!("EndpointReader is starting");
    let suspended = self.suspended.clone();

    let request_arc = Arc::new(Mutex::new(request));

    let (disconnect_tx, disconnect_rx) = mpsc::channel(1);
    let disconnect_tx_arc = Arc::new(Mutex::new(Some(disconnect_tx)));
    let disconnect_rx_arc = Arc::new(Mutex::new(disconnect_rx));

    let (response_tx, response_rx) = mpsc::channel::<Result<RemoteMessage, Status>>(100);

    let connection_key = RequestKeyWrapper::new(request_arc.clone());
    let endpoint_reader_connections = self.get_endpoint_manager().await.get_endpoint_reader_connections();
    endpoint_reader_connections.insert(connection_key.clone(), disconnect_tx_arc.clone());

    tokio::spawn({
      let cloned_self = self.clone();
      let cloned_disconnect_rx = disconnect_rx_arc.clone();
      let cloned_response_tx = response_tx.clone();
      let cloned_connection_key = connection_key.clone();
      async move {
        if Self::get_disconnect_flg(cloned_disconnect_rx).await {
          tracing::debug!("EndpointReader is telling to remote that it's leaving");
          if let Err(e) = cloned_response_tx
            .send(Ok(RemoteMessage {
              message_type: Some(remote::remote_message::MessageType::DisconnectRequest(
                remote::DisconnectRequest {},
              )),
            }))
            .await
          {
            tracing::error!("EndpointReader failed to send disconnection message: {}", e);
          }
        } else {
          cloned_self
            .get_endpoint_manager()
            .await
            .get_endpoint_reader_connections()
            .remove(&cloned_connection_key);
          tracing::debug!("EndpointReader removed active endpoint from endpointManager");
        }
      }
    });

    tokio::spawn({
      let cloned_self = self.clone();
      let cloned_request_arc = request_arc.clone();
      let cloned_response_tx = response_tx.clone();
      async move {
        let mut request_mg = cloned_request_arc.lock().await;
        while let Some(msg) = request_mg.get_mut().next().await {
          match msg {
            Ok(remote_msg) => {
              if suspended.load(Ordering::SeqCst) {
                continue;
              }

              match remote_msg.message_type {
                Some(message_type) => match message_type {
                  remote::remote_message::MessageType::ConnectRequest(connect_req) => {
                    if let Err(e) = cloned_self.on_connect_request(&cloned_response_tx, &connect_req).await {
                      tracing::error!("Failed to handle connect request, {}", e);
                      break;
                    }
                  }
                  remote::remote_message::MessageType::MessageBatch(message_batch) => {
                    if let Err(e) = cloned_self.on_message_batch(&message_batch).await {
                      tracing::error!("Failed to handle message batch, {}", e);
                      break;
                    }
                  }
                  _ => {
                    tracing::warn!("Received unknown message type");
                  }
                },
                None => {
                  tracing::warn!("Received message with no type");
                }
              }
            }
            Err(e) => {
              tracing::error!("Failed to receive message: {}", e);
              break;
            }
          }
        }

        if let Some(tx) = disconnect_tx_arc.lock().await.take() {
          let _ = tx.send(false).await;
        }
        tracing::info!("EndpointReader stream closed");
      }
    });

    let output_stream = ReceiverStream::new(response_rx);
    Ok(Response::new(Box::pin(output_stream) as Self::ReceiveStream))
  }

  async fn list_processes(&self, _: Request<ListProcessesRequest>) -> Result<Response<ListProcessesResponse>, Status> {
    Err(Status::unimplemented("Method not implemented"))
  }

  async fn get_process_diagnostics(
    &self,
    _: Request<GetProcessDiagnosticsRequest>,
  ) -> Result<Response<GetProcessDiagnosticsResponse>, Status> {
    Err(Status::unimplemented("Method not implemented"))
  }
}