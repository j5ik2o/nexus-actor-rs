use crate::actor::actor::{Actor, ActorError, ActorInnerError, SpawnError};
use crate::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SpawnerPart};
use crate::actor::message::{Message, ResponseHandle};
use crate::generated::remote::{ActorPidRequest, ActorPidResponse};
use crate::remote::messages::{Ping, Pong};
use crate::remote::remote::Remote;
use crate::remote::response_status_code::ResponseStatusCode;
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex;

impl Message for ActorPidRequest {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<ActorPidRequest>() {
      Some(a) => self == a,
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl Message for ActorPidResponse {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<ActorPidResponse>() {
      Some(a) => self == a,
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug)]
pub struct Activator {
  remote: Arc<Mutex<Remote>>,
}

impl Activator {
  pub fn new(remote: Arc<Mutex<Remote>>) -> Self {
    Activator { remote }
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
      let kinds = {
        let mg = self.remote.lock().await;
        mg.get_kinds().clone()
      };
      let props_opt = kinds.get(&msg.kind);
      return match props_opt {
        None => {
          context_handle
            .respond(ResponseHandle::new(ActorPidResponse {
              pid: None,
              status_code: ResponseStatusCode::ResponseStatusCodeERROR as i32,
            }))
            .await;
          Err(ActorError::ReceiveError(ActorInnerError::new("Actor not found", 0)))
        }
        Some(v) => {
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
          let pid_result = ctx.spawn_named(v.clone(), &format!("remote-{}", &name)).await;

          match pid_result {
            Ok(pid) => {
              context_handle
                .respond(ResponseHandle::new(ActorPidResponse {
                  pid: Some(pid.inner_pid),
                  status_code: ResponseStatusCode::ResponseStatusCodeOK as i32,
                }))
                .await;
              Ok(())
            }
            Err(e) => match e {
              SpawnError::ErrNameExists(pid) => {
                context_handle
                  .respond(ResponseHandle::new(ActorPidResponse {
                    pid: Some(pid.inner_pid),
                    status_code: ResponseStatusCode::ResponseStatusCodePROCESSNAMEALREADYEXIST as i32,
                  }))
                  .await;
                Err(ActorError::ReceiveError(ActorInnerError::new(
                  "Actor already exists",
                  0,
                )))
              }
              SpawnError::ErrPreStart(e) => {
                context_handle
                  .respond(ResponseHandle::new(ActorPidResponse {
                    pid: None,
                    status_code: e.reason().unwrap().code,
                  }))
                  .await;
                Err(ActorError::ReceiveError(ActorInnerError::new(
                  "Failed to spawn actor",
                  0,
                )))
              }
            },
          }
        }
      };
    }
    Ok(())
  }

  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("Started Activator");
    Ok(())
  }
}
