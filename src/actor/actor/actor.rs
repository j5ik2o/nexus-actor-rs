use std::fmt::Debug;

use crate::actor::actor::actor_error::ActorError;
use crate::actor::context::ContextHandle;
use crate::actor::context::MessagePart;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::TerminateInfo;
use crate::actor::supervisor::SupervisorStrategyHandle;
use async_trait::async_trait;
use tracing::instrument;

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  #[instrument(skip_all)]
  async fn handle(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let message_handle = context_handle.get_message_handle().await;
    let arm = message_handle.to_typed::<AutoReceiveMessage>();
    match arm {
      Some(arm) => match arm {
        AutoReceiveMessage::PreStart => self.pre_start(context_handle).await,
        AutoReceiveMessage::PostStart => self.post_start(context_handle).await,
        AutoReceiveMessage::PreRestart => self.pre_restart(context_handle).await,
        AutoReceiveMessage::PostRestart => self.post_restart(context_handle).await,
        AutoReceiveMessage::PreStop => self.pre_stop(context_handle).await,
        AutoReceiveMessage::PostStop => self.post_stop(context_handle).await,
        AutoReceiveMessage::Terminated(t) => self.post_child_terminate(context_handle, &t).await,
      },
      _ => self.receive(context_handle).await,
    }
  }

  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError>;

  #[instrument]
  async fn pre_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_start");
    Ok(())
  }

  #[instrument]
  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_start");
    Ok(())
  }

  #[instrument]
  async fn pre_restart(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_restart");
    Ok(())
  }

  #[instrument]
  async fn post_restart(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_restart");
    self.pre_start(context_handle).await
  }

  #[instrument]
  async fn pre_stop(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_stop");
    Ok(())
  }

  #[instrument]
  async fn post_stop(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_stop");
    Ok(())
  }

  #[instrument]
  async fn post_child_terminate(&self, _: ContextHandle, _: &TerminateInfo) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_child_terminate");
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}
