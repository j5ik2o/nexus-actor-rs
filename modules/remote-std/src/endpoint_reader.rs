use nexus_actor_core_rs::runtime::CoreTaskFuture;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::SenderPart;
use nexus_actor_std_rs::actor::core::{ActorProcess, ExtendedPid};
use nexus_actor_std_rs::actor::message::{MessageEnvelope, MessageHandle, MessageHeaders};
use nexus_actor_std_rs::actor::metrics::metrics_impl::MetricsSink;
use nexus_actor_std_rs::actor::process::Process;
use nexus_actor_std_rs::generated::actor::Pid;

use crate::endpoint_manager::{EndpointManager, RequestKeyWrapper};
use crate::generated::remote;
use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remoting_server::Remoting;
use crate::generated::remote::{
  ClientConnection, ConnectRequest, GetProcessDiagnosticsRequest, GetProcessDiagnosticsResponse,
  ListProcessesMatchType, ListProcessesRequest, ListProcessesResponse, MessageBatch, RemoteMessage, ServerConnection,
};
use crate::message_decoder::{decode_wire_message, DecodedMessage};
use crate::messages::RemoteTerminate;
use crate::remote::Remote;
use crate::serializer::SerializerId;
use opentelemetry::KeyValue;
use regex::Regex;
use std::collections::HashSet;
use std::convert::TryFrom;
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
  #[allow(dead_code)]
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

  pub(crate) async fn on_connect_request(
    &self,
    response_tx: &Sender<Result<RemoteMessage, Status>>,
    connect_req: &ConnectRequest,
    connection_key: Option<&RequestKeyWrapper>,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    match &connect_req.connection_type {
      Some(ConnectionType::ServerConnection(sc)) => {
        self.on_server_connection(response_tx, sc).await;
      }
      Some(ConnectionType::ClientConnection(cc)) => {
        self.on_client_connection(response_tx, cc, connection_key).await;
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

  async fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .metrics_sink()
      .await
  }

  async fn on_server_connection(&self, response_tx: &Sender<Result<RemoteMessage, Status>>, sc: &ServerConnection) {
    let blocked = self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_block_list()
      .is_blocked(&sc.system_id)
      .await;

    if blocked {
      tracing::debug!("EndpointReader blocked connection from {}", sc.system_id);
    } else {
      tracing::debug!("EndpointReader accepted connection from {}", sc.system_id);
    }

    if let Err(e) = self.send_connect_response(response_tx, &sc.system_id, blocked).await {
      tracing::error!("EndpointReader failed to send ConnectResponse message: {}", e);
    }

    if !blocked {
      if let Some(manager) = self.get_endpoint_manager_opt().await {
        manager.mark_heartbeat(&sc.system_id).await;
      }
    }
  }

  async fn on_client_connection(
    &self,
    response_tx: &Sender<Result<RemoteMessage, Status>>,
    cc: &ClientConnection,
    connection_key: Option<&RequestKeyWrapper>,
  ) {
    let blocked = self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_block_list()
      .is_blocked(&cc.system_id)
      .await;

    if blocked {
      tracing::debug!("EndpointReader blocked client connection from {}", cc.system_id);
    } else {
      tracing::debug!("EndpointReader accepted client connection from {}", cc.system_id);
    }

    if let Err(e) = self.send_connect_response(response_tx, &cc.system_id, blocked).await {
      tracing::error!("EndpointReader failed to send client ConnectResponse message: {}", e);
    }

    if !blocked {
      if let Some(key) = connection_key.cloned() {
        if let Some(manager) = self.get_endpoint_manager_opt().await {
          manager.register_client_connection(cc.system_id.clone(), key, response_tx.clone());
        } else {
          tracing::debug!(
            "EndpointManager not initialized; skipping client connection registration for {}",
            cc.system_id
          );
        }
      } else {
        tracing::debug!(
          "Client connection {} accepted without connection key; skipping EndpointManager registration",
          cc.system_id
        );
      }
    }

    if !blocked {
      if let Some(manager) = self.get_endpoint_manager_opt().await {
        manager.mark_heartbeat(&cc.system_id).await;
      }
    }
  }

  async fn send_connect_response(
    &self,
    response_tx: &Sender<Result<RemoteMessage, Status>>,
    system_id: &str,
    blocked: bool,
  ) -> Result<(), Status> {
    response_tx
      .send(Ok(RemoteMessage {
        message_type: Some(remote::remote_message::MessageType::ConnectResponse(
          remote::ConnectResponse {
            blocked,
            member_id: system_id.to_string(),
          },
        )),
      }))
      .await
      .map_err(|e| Status::internal(format!("failed to send connect response: {}", e)))
  }

  async fn on_message_batch(&self, message_batch: &MessageBatch) -> Result<(), EndpointReaderError> {
    tracing::debug!("EndpointReader received {} envelopes", message_batch.envelopes.len());

    let mut touched_addresses: HashSet<String> = HashSet::new();
    let metrics_sink = self.metrics_sink().await;
    for envelope in &message_batch.envelopes {
      let payload = &envelope.message_data;
      let mut sender = Pid::default();
      let mut target = Pid::default();

      let sender_opt = deserialize_sender(
        &mut sender,
        envelope.sender,
        envelope.sender_request_id,
        &message_batch.senders,
      );

      let target = deserialize_target(
        &mut target,
        envelope.target,
        envelope.target_request_id,
        &message_batch.targets,
      )
      .map(ExtendedPid::new)
      .ok_or(EndpointReaderError::UnknownTarget)?;
      touched_addresses.insert(target.address().to_string());

      let serializer_id =
        SerializerId::try_from(envelope.serializer_id).map_err(EndpointReaderError::Deserialization)?;

      if envelope.type_id < 0 || (envelope.type_id as usize) >= message_batch.type_names.len() {
        return Err(EndpointReaderError::Deserialization(format!(
          "Invalid type id: {}",
          envelope.type_id
        )));
      }

      let type_name = message_batch.type_names[envelope.type_id as usize].clone();

      if let Some(sink) = metrics_sink.as_ref() {
        let labels = vec![
          KeyValue::new("remote.endpoint", target.address().to_string()),
          KeyValue::new("remote.direction", "inbound".to_string()),
          KeyValue::new("remote.message_type", type_name.clone()),
        ];
        sink.record_message_size_with_labels(payload.len() as u64, &labels);
      }

      let target_addr = target.address().to_string();

      let decoded = match decode_wire_message(&type_name, &serializer_id, payload) {
        Ok(decoded) => decoded,
        Err(e) => {
          if let Some(sink) = metrics_sink.as_ref() {
            let labels = vec![
              KeyValue::new("remote.endpoint", target_addr.clone()),
              KeyValue::new("remote.direction", "inbound".to_string()),
              KeyValue::new("remote.message_type", type_name.clone()),
              KeyValue::new("remote.error", "deserialization".to_string()),
            ];
            sink.increment_remote_receive_failure_with_labels(&labels);
          }
          return Err(EndpointReaderError::Deserialization(e.to_string()));
        }
      };

      let actor_system = self.get_actor_system().await;
      let mut root_context = actor_system.get_root_context().await;
      let process_registry = actor_system.get_process_registry().await;
      let local_process = process_registry.get_local_process(target.id()).await;

      match decoded {
        DecodedMessage::Terminated(terminated) => {
          let remote_terminate = RemoteTerminate {
            watcher: Some(target.inner_pid.clone()),
            watchee: terminated.who.clone(),
          };
          self
            .get_endpoint_manager()
            .await
            .remote_terminate(&remote_terminate)
            .await;
        }
        DecodedMessage::System(system_message) => {
          let ref_process = local_process.clone().ok_or(EndpointReaderError::UnknownTarget)?;
          ref_process
            .send_system_message(&target, MessageHandle::new(system_message))
            .await;
        }
        DecodedMessage::User(message_arc) => {
          let msg_handle = MessageHandle::new_arc(message_arc);

          if sender_opt.is_none() && envelope.message_header.is_none() {
            root_context.send(target.clone(), msg_handle).await;
            if let Some(sink) = metrics_sink.as_ref() {
              let labels = vec![
                KeyValue::new("remote.endpoint", target_addr.clone()),
                KeyValue::new("remote.direction", "inbound".to_string()),
                KeyValue::new("remote.message_type", type_name.clone()),
                KeyValue::new("remote.result", "success".to_string()),
              ];
              sink.increment_remote_receive_success_with_labels(&labels);
            }
            continue;
          }

          let headers = envelope
            .message_header
            .as_ref()
            .map(|header| MessageHeaders::with_values(header.header_data.clone()))
            .unwrap_or_default();

          let mut local_envelope = MessageEnvelope::new(msg_handle).with_header(headers);
          if let Some(sender_pid) = sender_opt {
            local_envelope = local_envelope.with_sender(ExtendedPid::new(sender_pid));
          }
          root_context
            .send(target.clone(), MessageHandle::new(local_envelope))
            .await;
        }
      }

      if let Some(sink) = metrics_sink.as_ref() {
        let labels = vec![
          KeyValue::new("remote.endpoint", target_addr.clone()),
          KeyValue::new("remote.direction", "inbound".to_string()),
          KeyValue::new("remote.message_type", type_name.clone()),
          KeyValue::new("remote.result", "success".to_string()),
        ];
        sink.increment_remote_receive_success_with_labels(&labels);
      }
    }

    if let Some(manager) = self.get_endpoint_manager_opt().await {
      for address in touched_addresses {
        manager.mark_heartbeat(&address).await;
      }
    }
    Ok(())
  }

  pub fn set_suspend(&mut self, suspend: bool) {
    self.suspended.store(suspend, std::sync::atomic::Ordering::SeqCst);
  }

  #[cfg_attr(not(test), allow(dead_code))]
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

#[cfg(test)]
mod tests;

#[tonic::async_trait]
impl Remoting for EndpointReader {
  type ReceiveStream = Pin<Box<dyn Stream<Item = Result<RemoteMessage, Status>> + Send>>;

  async fn receive(&self, request: Request<Streaming<RemoteMessage>>) -> Result<Response<Self::ReceiveStream>, Status> {
    tracing::debug!("EndpointReader is starting");
    let suspended = self.suspended.clone();

    let request_arc = Arc::new(Mutex::new(request));

    let (disconnect_tx, disconnect_rx) = mpsc::channel(1);
    let disconnect_tx_arc = Arc::new(Mutex::new(Some(disconnect_tx)));
    let disconnect_rx_arc = Arc::new(Mutex::new(disconnect_rx));

    let (response_tx, response_rx) = mpsc::channel::<Result<RemoteMessage, Status>>(100);

    let connection_key = RequestKeyWrapper::new(request_arc.clone());
    let endpoint_reader_connections = self.get_endpoint_manager().await.get_endpoint_reader_connections();
    endpoint_reader_connections.insert(connection_key.clone(), disconnect_tx_arc.clone());

    let actor_system = self.get_actor_system().await;
    let spawner = actor_system.core_runtime().spawner();

    let future_disconnect: CoreTaskFuture = {
      let cloned_self = self.clone();
      let cloned_disconnect_rx = disconnect_rx_arc.clone();
      let cloned_response_tx = response_tx.clone();
      let cloned_connection_key = connection_key.clone();
      Box::pin(async move {
        let manager = cloned_self.get_endpoint_manager().await;
        let should_disconnect = Self::get_disconnect_flg(cloned_disconnect_rx).await;
        if should_disconnect {
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
          tracing::debug!("EndpointReader removed active endpoint from endpointManager");
        }
        manager.get_endpoint_reader_connections().remove(&cloned_connection_key);
        manager.deregister_client_connection(&cloned_connection_key);
      })
    };

    match spawner.clone().spawn(future_disconnect) {
      Ok(handle) => handle.detach(),
      Err(err) => tracing::error!(error = ?err, "Failed to spawn disconnect watcher task"),
    }

    let future_stream: CoreTaskFuture = {
      let cloned_self = self.clone();
      let cloned_request_arc = request_arc.clone();
      let cloned_response_tx = response_tx.clone();
      let cloned_connection_key = connection_key.clone();
      let suspended_flag = suspended.clone();
      let disconnect_tx_arc = disconnect_tx_arc.clone();
      Box::pin(async move {
        let mut request_mg = cloned_request_arc.lock().await;
        while let Some(msg) = request_mg.get_mut().next().await {
          match msg {
            Ok(remote_msg) => {
              if suspended_flag.load(Ordering::SeqCst) {
                continue;
              }

              match remote_msg.message_type {
                Some(message_type) => match message_type {
                  remote::remote_message::MessageType::ConnectRequest(connect_req) => {
                    if let Err(e) = cloned_self
                      .on_connect_request(&cloned_response_tx, &connect_req, Some(&cloned_connection_key))
                      .await
                    {
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
        tracing::debug!("EndpointReader stream closed");
      })
    };

    match spawner.spawn(future_stream) {
      Ok(handle) => handle.detach(),
      Err(err) => tracing::error!(error = ?err, "Failed to spawn stream reader task"),
    }

    let output_stream = ReceiverStream::new(response_rx);
    Ok(Response::new(Box::pin(output_stream) as Self::ReceiveStream))
  }

  async fn list_processes(
    &self,
    request: Request<ListProcessesRequest>,
  ) -> Result<Response<ListProcessesResponse>, Status> {
    let req = request.into_inner();
    let match_type = ListProcessesMatchType::try_from(req.r#type).unwrap_or(ListProcessesMatchType::MatchPartOfString);

    let actor_system = self.get_actor_system().await;
    let registry = actor_system.get_process_registry().await;
    let all_pids = registry.list_local_pids().await;

    let pattern = req.pattern;
    let regex = if matches!(match_type, ListProcessesMatchType::MatchRegex) && !pattern.is_empty() {
      Some(Regex::new(&pattern).map_err(|e| Status::invalid_argument(format!("invalid regex '{}': {}", pattern, e)))?)
    } else {
      None
    };

    let mut filtered = Vec::new();
    for pid in all_pids {
      let matched = if pattern.is_empty() {
        true
      } else {
        match match_type {
          ListProcessesMatchType::MatchPartOfString => pid.id.contains(&pattern),
          ListProcessesMatchType::MatchExactString => pid.id == pattern,
          ListProcessesMatchType::MatchRegex => regex.as_ref().expect("regex should be compiled").is_match(&pid.id),
        }
      };

      if matched {
        filtered.push(pid);
      }
    }

    Ok(Response::new(ListProcessesResponse { pids: filtered }))
  }

  async fn get_process_diagnostics(
    &self,
    request: Request<GetProcessDiagnosticsRequest>,
  ) -> Result<Response<GetProcessDiagnosticsResponse>, Status> {
    let req = request.into_inner();
    let pid = req.pid.ok_or_else(|| Status::invalid_argument("pid is required"))?;

    let actor_system = self.get_actor_system().await;
    let current_address = actor_system.get_address().await;
    if pid.address != current_address {
      return Err(Status::invalid_argument("pid does not belong to this node"));
    }

    let registry = actor_system.get_process_registry().await;
    let process = registry
      .find_local_process_handle(&pid.id)
      .await
      .ok_or_else(|| Status::not_found("process not found"))?;

    let diagnostics = if let Some(actor_process) = process.as_any().downcast_ref::<ActorProcess>() {
      format!("ActorProcess(dead={})", actor_process.is_dead())
    } else {
      "Process diagnostics not available".to_string()
    };

    Ok(Response::new(GetProcessDiagnosticsResponse {
      diagnostics_string: diagnostics,
    }))
  }
}
