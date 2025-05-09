use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::SenderPart;
use nexus_actor_core_rs::actor::core::ExtendedPid;
use nexus_actor_core_rs::actor::message::{MessageEnvelope, MessageHandle, MessageHeaders, SystemMessage};
use nexus_actor_core_rs::actor::process::Process;
use nexus_actor_core_rs::generated::actor::{Pid, Stop, Terminated, Unwatch, Watch};
use std::any::Any as StdAny;

use crate::endpoint_manager::{EndpointManager, RequestKeyWrapper};
use crate::generated::cluster::{DeliverBatchRequestTransport, PubSubAutoRespondBatchTransport, PubSubBatchTransport};
use crate::generated::remote;
use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remoting_server::Remoting;
use crate::generated::remote::{
  ConnectRequest, GetProcessDiagnosticsRequest, GetProcessDiagnosticsResponse, ListProcessesRequest,
  ListProcessesResponse, MessageBatch, RemoteMessage, ServerConnection,
};
use crate::remote::Remote;
use crate::serializer::{deserialize_any, deserialize_message, SerializerError, SerializerId};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};
use tonic::codegen::tokio_stream::wrappers::ReceiverStream;
use tonic::codegen::tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointReaderError {
  #[error("Unknown target")]
  UnknownTarget,
  #[error("Unknown sender")]
  UnknownSender,
  #[error("Deserialization error: {0}")]
  Deserialization(String),
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
    let type_pub_sub_batch: &'static str = std::any::type_name::<PubSubBatchTransport>();
    let type_deliver_batch: &'static str = std::any::type_name::<DeliverBatchRequestTransport>();
    let type_pub_sub_auto_respond: &'static str = std::any::type_name::<PubSubAutoRespondBatchTransport>();

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
        EndpointReaderError::UnknownTarget
      })?;

      let serializer_id = SerializerId::try_from(envelope.serializer_id).expect("Invalid serializer id");

      if !data.is_empty() {
        let preview_len = std::cmp::min(data.len(), 16);
        tracing::debug!("Data preview: {:?}", &data[..preview_len]);
      }

      let result = match deserialize_any(data, &serializer_id, type_pub_sub_batch) {
        Ok(v) => {
          tracing::debug!("Successfully deserialized as PubSubBatchTransport");
          Some(v)
        }
        Err(e1) => {
          tracing::debug!("Failed to deserialize as PubSubBatchTransport: {:?}", e1);
          match deserialize_any(data, &serializer_id, type_deliver_batch) {
            Ok(v) => {
              tracing::debug!("Successfully deserialized as DeliverBatchRequestTransport");
              Some(v)
            }
            Err(e2) => {
              tracing::debug!("Failed to deserialize as DeliverBatchRequestTransport: {:?}", e2);
              match deserialize_any(data, &serializer_id, type_pub_sub_auto_respond) {
                Ok(v) => {
                  tracing::debug!("Successfully deserialized as PubSubAutoRespondBatchTransport");
                  Some(v)
                }
                Err(e3) => {
                  tracing::debug!("Failed to deserialize as PubSubAutoRespondBatchTransport: {:?}", e3);
                  None
                }
              }
            }
          }
        }
      };

      tracing::debug!("result = {:?}", result);

      let actor_system = self.get_actor_system().await;
      let mut root_context = actor_system.get_root_context().await;
      let process_registry = actor_system.get_process_registry().await;
      let local_process = process_registry.get_local_process(target.id()).await;

      match result {
        Some(message) => {
          if let Some(t) = message.downcast_ref::<Terminated>() {
            let terminated = SystemMessage::of_terminate(t.clone());
            root_context.send(target.clone(), MessageHandle::new(terminated)).await;
          }
          if message.downcast_ref::<Stop>().is_some() {
            let system_message = SystemMessage::of_stop();
            let ref_process = local_process.clone().ok_or(EndpointReaderError::UnknownTarget)?;
            ref_process
              .send_system_message(&target, MessageHandle::new(system_message))
              .await;
          }
          if let Some(watch) = message.downcast_ref::<Watch>() {
            let system_message = SystemMessage::of_watch(watch.clone());
            let ref_process = local_process.clone().ok_or(EndpointReaderError::UnknownTarget)?;
            ref_process
              .send_system_message(&target, MessageHandle::new(system_message))
              .await;
          }
          if let Some(unwatch) = message.downcast_ref::<Unwatch>() {
            let system_message = SystemMessage::of_unwatch(unwatch.clone());
            let ref_process = local_process.ok_or(EndpointReaderError::UnknownTarget)?;
            ref_process
              .send_system_message(&target, MessageHandle::new(system_message))
              .await;
          }
        }
        None => {
          let type_name = if envelope.type_id >= 0 && (envelope.type_id as usize) < message_batch.type_names.len() {
            &message_batch.type_names[envelope.type_id as usize]
          } else {
            tracing::warn!("Invalid type_id: {}, using default type name", envelope.type_id);
            "unknown"
          };
          let data_arc = deserialize_message(data, &serializer_id, type_name)
            .map_err(|e| EndpointReaderError::Deserialization(e.to_string()))?;
          let msg_handle = MessageHandle::new_arc(data_arc.clone());
          tracing::info!("EndpointReader received message: {:?}", data_arc);

          if sender_opt.is_none() && envelope.message_header.is_none() {
            tracing::info!("EndpointReader received message with no sender and no header");
            root_context.send(target, msg_handle).await;
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
          root_context.send(target, MessageHandle::new(local_me)).await;
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
    disconnect_mg.recv().await.unwrap_or_default()
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

fn deserialize_sender(pid: &mut Pid, index: i32, request_id: u32, arr: &[Pid]) -> Option<Pid> {
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

fn deserialize_target(pid: &mut Pid, index: i32, request_id: u32, arr: &[Pid]) -> Option<Pid> {
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
