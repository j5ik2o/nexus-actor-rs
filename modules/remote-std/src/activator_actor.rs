use crate::generated::remote::{ActorPidRequest, ActorPidResponse};
use crate::messages::{Ping, Pong};
use crate::metrics::record_sender_snapshot;
use crate::remote::Remote;
use crate::response_status_code::ResponseStatusCode;
use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid, Props, SpawnError};
use nexus_actor_std_rs::actor::message::{MessageHandle, ResponseHandle};
use nexus_actor_std_rs::actor::process::actor_future::ActorFuture;
use nexus_actor_std_rs::actor::process::future::ActorFutureError;
use nexus_actor_std_rs::generated::actor::Pid;
use std::sync::Weak;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Activator {
  remote: Weak<Remote>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ActivatorError {
  #[error("Failed to spawn actor")]
  SpawnError(ActorFutureError),
  #[error("Failed to get actor system: {0}")]
  ActorPidResponseError(String),
}

impl Activator {
  pub fn new(remote: Weak<Remote>) -> Self {
    Activator { remote }
  }

  fn get_kinds(&self) -> DashMap<String, Props> {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_kinds()
      .clone()
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_actor_system()
      .clone()
  }

  fn activator_for_address(&self, address: &str) -> Pid {
    Pid::new(address, "activator")
  }

  pub async fn spawn_future(&self, address: &str, name: &str, kind: &str, timeout: Duration) -> ActorFuture {
    let activator = self.activator_for_address(address);
    let actor_system = self.get_actor_system().await;
    let root_context = actor_system.get_root_context().await;
    let pid = ExtendedPid::new(activator.clone());
    root_context
      .request_future(
        pid,
        MessageHandle::new(ActorPidRequest {
          kind: kind.to_string(),
          name: name.to_string(),
        }),
        timeout,
      )
      .await
  }

  pub async fn spawn_named(
    &self,
    address: &str,
    name: &str,
    kind: &str,
    timeout: Duration,
  ) -> Result<ActorPidResponse, ActivatorError> {
    let f = self.spawn_future(address, name, kind, timeout).await;
    let result = f.result().await;
    match result {
      Ok(response) => {
        if let Some(response) = response.to_typed::<ActorPidResponse>() {
          Ok(response)
        } else {
          Err(ActivatorError::ActorPidResponseError(
            "Failed to get actor pid response".to_string(),
          ))
        }
      }
      Err(e) => {
        tracing::error!("Failed to spawn actor {}: {:?}", name, e);
        Err(ActivatorError::SpawnError(e))
      }
    }
  }

  pub async fn spawn(&self, address: &str, kind: &str, timeout: Duration) -> Result<ActorPidResponse, ActivatorError> {
    self.spawn_named(address, "", kind, timeout).await
  }
}

#[async_trait]
impl Actor for Activator {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let _ = record_sender_snapshot(&context_handle).await;
    let message_handle = if let Some(handle) = context_handle.try_get_message_handle_opt() {
      handle
    } else {
      context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
    };
    if message_handle.is_typed::<Ping>() {
      context_handle.respond(ResponseHandle::new(Pong {})).await;
    }
    if let Some(msg) = message_handle.to_typed::<ActorPidRequest>() {
      if let Some(remote) = self.remote.upgrade() {
        if let Some(handler) = remote.activation_handler().await {
          let identity_name = if msg.name.is_empty() {
            context_handle
              .get_actor_system()
              .await
              .get_process_registry()
              .await
              .next_id()
          } else {
            msg.name.clone()
          };

          match handler.activate(&msg.kind, &identity_name).await {
            Ok(Some(pid)) => {
              context_handle
                .respond(ResponseHandle::new(ActorPidResponse {
                  pid: Some(pid),
                  status_code: ResponseStatusCode::Ok.to_i32(),
                }))
                .await;
              return Ok(());
            }
            Ok(None) => {
              context_handle
                .respond(ResponseHandle::new(ActorPidResponse {
                  pid: None,
                  status_code: ResponseStatusCode::Error.to_i32(),
                }))
                .await;
              return Err(ActorError::ReceiveError(ErrorReason::new("Actor not found", 0)));
            }
            Err(err) => {
              context_handle
                .respond(ResponseHandle::new(ActorPidResponse {
                  pid: None,
                  status_code: err.status_code().to_i32(),
                }))
                .await;
              return Err(ActorError::ReceiveError(ErrorReason::new(
                "Activation handler failed",
                0,
              )));
            }
          }
        }
      }

      return match self.get_kinds().get(&msg.kind) {
        None => {
          context_handle
            .respond(ResponseHandle::new(ActorPidResponse {
              pid: None,
              status_code: ResponseStatusCode::Error.to_i32(),
            }))
            .await;
          Err(ActorError::ReceiveError(ErrorReason::new("Actor not found", 0)))
        }
        Some(props) => {
          let mut name = msg.name;
          if name.is_empty() {
            name = context_handle
              .get_actor_system()
              .await
              .get_process_registry()
              .await
              .next_id();
          }
          let mut ctx = context_handle.clone();
          match ctx.spawn_named(props.clone(), &format!("remote-{}", &name)).await {
            Ok(pid) => {
              context_handle
                .respond(ResponseHandle::new(ActorPidResponse {
                  pid: Some(pid.inner_pid),
                  status_code: ResponseStatusCode::Ok.to_i32(),
                }))
                .await;
              Ok(())
            }
            Err(spawn_error) => match spawn_error {
              SpawnError::ErrNameExists(pid) => {
                context_handle
                  .respond(ResponseHandle::new(ActorPidResponse {
                    pid: Some(pid.inner_pid),
                    status_code: ResponseStatusCode::ProcessNameAlreadyExists.to_i32(),
                  }))
                  .await;
                Err(ActorError::ReceiveError(ErrorReason::new("Actor already exists", 0)))
              }
              SpawnError::ErrPreStart(actor_error) => {
                let status_code = actor_error
                  .reason()
                  .and_then(|reason| ResponseStatusCode::from_i32(reason.code()))
                  .unwrap_or(ResponseStatusCode::Error)
                  .to_i32();
                context_handle
                  .respond(ResponseHandle::new(ActorPidResponse { pid: None, status_code }))
                  .await;
                Err(ActorError::ReceiveError(ErrorReason::new("Failed to spawn actor", 0)))
              }
            },
          }
        }
      };
    }
    Ok(())
  }

  async fn post_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("Started Activator");
    Ok(())
  }
}
