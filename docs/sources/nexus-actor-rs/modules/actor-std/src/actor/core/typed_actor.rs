use crate::actor::context::{ContextHandle, TypedContextHandle};
use crate::actor::core::{Actor, ActorError};
use crate::actor::message::{AutoReceiveMessage, Message, TerminatedMessage};
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
    let message_handle = if let Some(handle) = context_handle
      .get_underlying()
      .with_typed_borrow::<M, _, _>(|view| view.message_handle())
      .flatten()
    {
      handle
    } else {
      context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
    };
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

  //#[instrument]
  async fn pre_start(&mut self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_start");
    Ok(())
  }

  //#[instrument]
  async fn post_start(&mut self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_start");
    Ok(())
  }

  //#[instrument]
  async fn pre_restart(&mut self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_restart");
    Ok(())
  }

  //#[instrument]
  async fn post_restart(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_restart");
    self.pre_start(context_handle).await
  }

  //#[instrument]
  async fn pre_stop(&mut self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::pre_stop");
    Ok(())
  }

  //#[instrument]
  async fn post_stop(&mut self, _: TypedContextHandle<M>) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_stop");
    Ok(())
  }

  //#[instrument]
  async fn post_child_terminate(&mut self, _: TypedContextHandle<M>, _: &TerminatedMessage) -> Result<(), ActorError> {
    tracing::debug!("Actor::post_child_terminate");
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
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

  async fn pre_start(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.pre_start(typed_context_handle).await
  }

  async fn post_start(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_start(typed_context_handle).await
  }

  async fn pre_restart(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.pre_restart(typed_context_handle).await
  }

  async fn post_restart(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_restart(typed_context_handle).await
  }

  async fn pre_stop(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.pre_stop(typed_context_handle).await
  }

  async fn post_stop(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_stop(typed_context_handle).await
  }

  async fn post_child_terminate(
    &mut self,
    context_handle: ContextHandle,
    terminated: &TerminatedMessage,
  ) -> Result<(), ActorError> {
    let typed_context_handle = TypedContextHandle::new(context_handle);
    self.actor.post_child_terminate(typed_context_handle, terminated).await
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    self.actor.get_supervisor_strategy().await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{ActorContext, Context, ContextCell, ContextHandle};
  use crate::actor::core::{ActorError, Props, TypedActor};
  use crate::actor::message::MessageHandle;
  use crate::actor::typed_context::{TypedContextSyncView, TypedMessagePart};
  use nexus_message_derive_rs::Message;
  use std::sync::{Arc, Mutex};
  use tokio::sync::RwLock;

  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  struct TestMessage;

  #[derive(Debug, Default)]
  struct RecordingActor {
    messages: Arc<Mutex<Vec<TestMessage>>>,
  }

  impl RecordingActor {
    fn new() -> Self {
      Self::default()
    }

    fn messages(&self) -> Arc<Mutex<Vec<TestMessage>>> {
      Arc::clone(&self.messages)
    }
  }

  #[async_trait]
  impl TypedActor<TestMessage> for RecordingActor {
    async fn receive(&mut self, context_handle: TypedContextHandle<TestMessage>) -> Result<(), ActorError> {
      if let Some(message) = context_handle.get_message_opt().await {
        self.messages.lock().unwrap().push(message);
      }
      Ok(())
    }

    async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
      None
    }
  }

  #[tokio::test]
  async fn handle_uses_async_fallback_when_snapshot_missing() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let props = Props::from_async_actor_receiver(|_ctx| async { Ok::<(), ActorError>(()) }).await;
    let actor_context = ActorContext::new(actor_system.clone(), props, None).await;
    actor_context
      .inject_message_for_test(MessageHandle::new(TestMessage.clone()))
      .await;

    let boxed: Box<dyn Context> = Box::new(actor_context.clone());
    let context_arc: Arc<RwLock<Box<dyn Context>>> = Arc::new(RwLock::new(boxed));
    let context_handle = ContextHandle::new_arc(context_arc, Arc::new(ContextCell::default()));
    let typed_handle = TypedContextHandle::<TestMessage>::new(context_handle.clone());

    // Snapshot should miss because no ActorContext was captured into the cell.
    let snapshot = typed_handle.sync_view();
    assert!(snapshot.message_handle_snapshot().is_none());

    let mut actor = RecordingActor::new();
    actor.handle(typed_handle).await.expect("actor handle");

    let messages = actor.messages();
    let guard = messages.lock().unwrap();
    assert_eq!(guard.as_slice(), &[TestMessage]);
  }
}
