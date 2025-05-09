use crate::generated::remote::{ActorPidRequest, ActorPidResponse};
use crate::messages::{Ping, Pong};
use crate::remote::Remote;
use crate::response_status_code::ResponseStatusCode;
use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid, Props, SpawnError};
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::dispatch::future::{ActorFuture, ActorFutureError};
use nexus_actor_core_rs::actor::message::{MessageHandle, ResponseHandle};
use nexus_actor_core_rs::generated::actor::Pid;
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
    addres: &str,
    name: &str,
    kind: &str,
    timeout: Duration,
  ) -> Result<ActorPidResponse, ActivatorError> {
    let f = self.spawn_future(addres, name, kind, timeout).await;
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
    let message_handle = context_handle.get_message_handle().await;
    if message_handle.is_typed::<Ping>() {
      context_handle.respond(ResponseHandle::new(Pong {})).await;
    }
    if let Some(msg) = message_handle.to_typed::<ActorPidRequest>() {
      return match self.get_kinds().get(&msg.kind) {
        None => {
          context_handle
            .respond(ResponseHandle::new(ActorPidResponse {
              pid: None,
              status_code: ResponseStatusCode::Error as i32,
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
                  status_code: ResponseStatusCode::Ok as i32,
                }))
                .await;
              Ok(())
            }
            Err(spawn_error) => match spawn_error {
              SpawnError::ErrNameExists(pid) => {
                context_handle
                  .respond(ResponseHandle::new(ActorPidResponse {
                    pid: Some(pid.inner_pid),
                    status_code: ResponseStatusCode::ProcessNameAlreadyExists as i32,
                  }))
                  .await;
                Err(ActorError::ReceiveError(ErrorReason::new("Actor already exists", 0)))
              }
              SpawnError::ErrPreStart(actor_error) => {
                context_handle
                  .respond(ResponseHandle::new(ActorPidResponse {
                    pid: None,
                    status_code: actor_error.reason().unwrap().code,
                  }))
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
