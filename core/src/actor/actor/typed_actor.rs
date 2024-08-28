use crate::actor::actor::{Actor, ActorError};
use crate::actor::context::{ContextHandle, TypedContextHandle};
use crate::actor::message::{AutoReceiveMessage, Message, TerminateInfo};
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::actor::typed_context::TypedMessagePart;
use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;
use tracing::instrument;

#[async_trait]
pub trait TypedActor<M: Message + Clone>: Debug + Send + Sync + 'static {
  #[instrument(skip_all)]
  async fn handle(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
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

  async fn receive(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError>;

  #[instrument]
  async fn pre_start(&self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_start");
    Ok(())
  }

  #[instrument]
  async fn post_start(&self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_start");
    Ok(())
  }

  #[instrument]
  async fn pre_restart(&self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_restart");
    Ok(())
  }

  #[instrument]
  async fn post_restart(&self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_restart");
    self.pre_start(context_handle).await
  }

  #[instrument]
  async fn pre_stop(&self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_stop");
    Ok(())
  }

  #[instrument]
  async fn post_stop(&self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_stop");
    Ok(())
  }

  #[instrument]
  async fn post_child_terminate(&self, _: TypedContextHandle<M>, _: &TerminateInfo) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_child_terminate");
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Debug, Clone)]
pub struct TypedActorWrapper<A: TypedActor<M>, M: Message + Clone> {
  actor: A,
  phantom_data: PhantomData<M>,
}

impl<A: TypedActor<M>, M: Message + Clone> TypedActorWrapper<A, M> {
  pub fn new(actor: A) -> Self {
    Self {
      actor,
      phantom_data: PhantomData,
    }
  }
}

#[async_trait]
impl<A: TypedActor<M>, M: Message + Clone> Actor for TypedActorWrapper<A, M> {
  #[instrument(skip_all)]
  async fn handle(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.handle(typed_context_handle).await
  }

  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.receive(typed_context_handle).await
  }

  async fn pre_start(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.pre_start(typed_context_handle).await
  }

  async fn post_start(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_start(typed_context_handle).await
  }

  async fn pre_restart(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.pre_restart(typed_context_handle).await
  }

  async fn post_restart(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_restart(typed_context_handle).await
  }

  async fn pre_stop(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.pre_stop(typed_context_handle).await
  }

  async fn post_stop(&self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_stop(typed_context_handle).await
  }

  async fn post_child_terminate(
    &self,
    context_handle: ContextHandle,
    terminate_info: &TerminateInfo,
  ) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self
      .actor
      .post_child_terminate(typed_context_handle, terminate_info)
      .await
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    self.actor.get_supervisor_strategy().await
  }
}