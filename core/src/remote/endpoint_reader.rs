use crate::generated::actor::Pid;
use crate::generated::remote;
use crate::generated::remote::connect_request::ConnectionType;
use crate::generated::remote::remoting_server::Remoting;
use crate::generated::remote::{
  ConnectRequest, GetProcessDiagnosticsRequest, GetProcessDiagnosticsResponse, ListProcessesRequest,
  ListProcessesResponse, MessageBatch, RemoteMessage, ServerConnection,
};
use crate::remote::remote::Remote;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, Mutex};
use tonic::codegen::tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

#[derive(Debug, Clone)]
pub(crate) struct EndpointReader {
  suspended: bool,
  remote: Arc<Mutex<Remote>>,
}

impl EndpointReader {
  pub(crate) fn new(remote: Arc<Mutex<Remote>>) -> Self {
    EndpointReader {
      suspended: false,
      remote,
    }
  }

  async fn on_server_connection(
    remote: Arc<Mutex<Remote>>,
    sc: &ServerConnection,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let remote_mg = remote.lock().await;

    if remote_mg.get_block_list().is_blocked(&sc.system_id).await {
      // Send blocked response
      // Note: Implementation of sending response needs to be added
    } else {
      // Send not blocked response
      // Note: Implementation of sending response needs to be added
    }
    Ok(())
  }

  async fn on_message_batch(
    remote: Arc<Mutex<Remote>>,
    batch: &MessageBatch,
  ) -> Result<(), Box<dyn std::error::Error>> {
    // for envelope in &batch.envelopes {
    //     // Deserialize sender and target
    //     // Process message based on its type
    //     // Note: Detailed implementation of message processing needs to be added
    // }
    Ok(())
  }

  async fn get_suspend(suspended: Arc<Mutex<bool>>) -> bool {
    *suspended.lock().await
  }
}

fn deserialize_sender<'a>(pid: &'a mut Pid, index: usize, request_id: u32, arr: &[Pid]) -> Option<&'a mut Pid> {
  if index == 0 {
    None
  } else {
    *pid = arr[index - 1].clone();
    if request_id > 0 {
      *pid = pid.clone();
      pid.request_id = request_id;
      Some(pid)
    } else {
      Some(pid)
    }
  }
}

fn deserialize_target<'a>(pid: &'a mut Pid, index: usize, request_id: u32, arr: &[Pid]) -> Option<&'a mut Pid> {
  *pid = arr[index].clone();
  if request_id > 0 {
    *pid = pid.clone();
    pid.request_id = request_id;
    Some(pid)
  } else {
    Some(pid)
  }
}

pub struct EndpointReaderResponse {
  rx: mpsc::Receiver<bool>,
}

impl Stream for EndpointReaderResponse {
  type Item = Result<RemoteMessage, Status>;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    match self.rx.poll_recv(cx) {
      Poll::Ready(Some(msg)) => Poll::Ready(Some(Ok(RemoteMessage::default()))),
      Poll::Ready(None) => Poll::Ready(None),
      Poll::Pending => Poll::Pending,
    }
  }
}

#[tonic::async_trait]
impl Remoting for EndpointReader {
  type ReceiveStream = EndpointReaderResponse;

  async fn receive(&self, request: Request<Streaming<RemoteMessage>>) -> Result<Response<Self::ReceiveStream>, Status> {
    let mut stream = request.into_inner();
    let (tx, rx) = mpsc::channel(100);
    let disconnect_chan = Arc::new(Mutex::new(Some(tx)));

    let remote = Arc::clone(&self.remote);
    let suspended = Arc::new(Mutex::new(self.suspended));

    tokio::spawn(async move {
      while let Some(msg) = stream.next().await {
        match msg {
          Ok(remote_msg) => {
            if Self::get_suspend(suspended.clone()).await {
              continue;
            }

            match remote_msg.message_type {
              Some(message_type) => match message_type {
                remote::remote_message::MessageType::ConnectRequest(connect_req) => match connect_req.connection_type {
                  Some(ConnectionType::ServerConnection(sc)) => {
                    if let Err(e) = Self::on_server_connection(remote.clone(), &sc).await {
                      tracing::error!("Failed to handle connect request: {}", e);
                      break;
                    }
                  }
                  _ => {
                    tracing::warn!("Received unknown connection type");
                  }
                },
                remote::remote_message::MessageType::MessageBatch(message_batch) => {
                  if let Err(e) = Self::on_message_batch(remote.clone(), &message_batch).await {
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

      if let Some(tx) = disconnect_chan.lock().await.take() {
        let _ = tx.send(false).await;
      }
      tracing::info!("EndpointReader stream closed");
    });

    Ok(Response::new(EndpointReaderResponse { rx }))
  }

  async fn list_processes(
    &self,
    request: Request<ListProcessesRequest>,
  ) -> Result<Response<ListProcessesResponse>, Status> {
    Err(Status::unimplemented("Method not implemented"))
  }

  async fn get_process_diagnostics(
    &self,
    request: Request<GetProcessDiagnosticsRequest>,
  ) -> Result<Response<GetProcessDiagnosticsResponse>, Status> {
    Err(Status::unimplemented("Method not implemented"))
  }
}
