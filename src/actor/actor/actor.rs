use std::fmt::Debug;

use async_trait::async_trait;

use crate::actor::actor::actor_error::ActorError;
use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::Terminated;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::MessagePart;
use crate::actor::message::auto_receive_message::AutoReceiveMessage;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::message_or_envelope::{unwrap_envelope_message, MessageEnvelope};
use crate::actor::message::system_message::SystemMessage;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  async fn handle(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    if let Some(message_handle) = context_handle.get_message_handle_opt().await {
      tracing::debug!("Actor::handle: message_handle = {:?}", message_handle);
      let me = message_handle.to_typed::<MessageEnvelope>();
      let sm = message_handle.to_typed::<SystemMessage>();
      let arm = message_handle.to_typed::<AutoReceiveMessage>();
      let t = message_handle.to_typed::<Terminated>();
      match (me, sm, arm, t) {
        (Some(_), None, None, None) => {
          let message = unwrap_envelope_message(message_handle.clone());
          tracing::debug!("Actor::handle: MessageEnvelope = {:?}", message);
          self.receive(context_handle.clone(), message).await
        }
        (None, Some(sm), None, None) => match sm {
          SystemMessage::Started => self.post_start(context_handle).await,
          _ => Ok(()),
        },
        (None, None, Some(arm), None) => match arm {
          AutoReceiveMessage::PreRestart => self.pre_restart(context_handle).await,
          AutoReceiveMessage::PreStop => self.pre_stop(context_handle).await,
          AutoReceiveMessage::PostStop => self.post_stop(context_handle).await,
          AutoReceiveMessage::PoisonPill => Ok(()),
        },
        (None, None, None, Some(t)) => self.on_child_terminated(context_handle, &t).await,
        _ => self.receive(context_handle.clone(), message_handle).await,
      }
    } else {
      tracing::error!("No message found");
      Err(ActorError::ReceiveError(ActorInnerError::new(
        "No message found".to_string(),
      )))
    }
  }

  async fn receive(&mut self, context_handle: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError>;

  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::started");
    Ok(())
  }

  async fn pre_restart(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::restarting");
    Ok(())
  }

  async fn pre_stop(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::stopping");
    Ok(())
  }

  async fn post_stop(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::stopped");
    Ok(())
  }

  async fn on_child_terminated(&self, _: ContextHandle, _: &Terminated) -> Result<(), ActorError> {
    tracing::debug!("Actor::on_child_terminated");
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}
