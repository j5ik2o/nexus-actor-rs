use crate::actor::actor::actor_error::ActorError;
use crate::actor::context::ContextHandle;
use crate::actor::context::MessagePart;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::actor::Config;
use crate::generated::actor::Terminated;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }

  // #[instrument(skip_all)]
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

  //#[instrument]
  async fn pre_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_start");
    Ok(())
  }

  //#[instrument]
  async fn post_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_start");
    Ok(())
  }

  //#[instrument]
  async fn pre_restart(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_restart");
    Ok(())
  }

  //#[instrument]
  async fn post_restart(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_restart");
    self.pre_start(context_handle).await
  }

  //#[instrument]
  async fn pre_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_stop");
    Ok(())
  }

  //#[instrument]
  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_stop");
    Ok(())
  }

  //#[instrument]
  async fn post_child_terminate(&mut self, _: ContextHandle, _: &Terminated) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_child_terminate");
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

pub struct ActorConfigOption(Arc<Mutex<dyn FnMut(&mut Config) + Send + Sync + 'static>>);

impl ActorConfigOption {
  pub fn new<F>(f: F) -> Self
  where
    F: FnMut(&mut Config) + Send + Sync + 'static, {
    Self(Arc::new(Mutex::new(f)))
  }

  pub async fn run(&self, config: &mut Config) {
    let mut mg = self.0.lock().await;
    mg(config)
  }

  async fn configure(options: Vec<ActorConfigOption>) -> Config {
    let mut config = Config::default();
    for option in options {
      option.run(&mut config).await;
    }
    config
  }
}
