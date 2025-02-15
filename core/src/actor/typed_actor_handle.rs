use crate::actor::typed_actor::TypedActor;
use crate::actor::{Actor, ActorError, ActorHandle};
use crate::actor::context::TypedContextHandle;
use crate::actor::message::Message;
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::generated::actor::Terminated;
use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct TypedActorHandle<M: Message>(Arc<Mutex<dyn TypedActor<M>>>);

impl<M: Message> PartialEq for TypedActorHandle<M> {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl<M: Message> Eq for TypedActorHandle<M> {}

impl<M: Message> std::hash::Hash for TypedActorHandle<M> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn TypedActor<M>>).hash(state);
  }
}

impl<M: Message + Clone> TypedActorHandle<M> {
  pub fn new_arc(actor: Arc<Mutex<dyn TypedActor<M>>>) -> Self {
    TypedActorHandle(actor)
  }

  pub fn new(actor: impl TypedActor<M> + 'static) -> Self {
    TypedActorHandle(Arc::new(Mutex::new(actor)))
  }
}

#[async_trait]
impl<M: Message + Clone> TypedActor<M> for TypedActorHandle<M> {
  async fn handle(&mut self, c: TypedContextHandle<M>) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.handle(c).await
  }

  async fn receive(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(context_handle).await
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    let mut mg = self.0.lock().await;
    mg.get_supervisor_strategy().await
  }
}

#[derive(Debug, Clone)]
pub struct TypeWrapperActorHandle<M: Message> {
  underlying: ActorHandle,
  phantom_data: PhantomData<M>,
}

impl<M: Message> TypeWrapperActorHandle<M> {
  pub fn new(underlying: ActorHandle) -> Self {
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }
}

#[async_trait]
impl<M: Message + Clone> TypedActor<M> for TypeWrapperActorHandle<M> {
  async fn handle(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self.underlying.handle(context_handle.get_underlying().clone()).await
  }

  async fn receive(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self.underlying.receive(context_handle.get_underlying().clone()).await
  }

  async fn pre_start(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self.underlying.pre_start(context_handle.get_underlying().clone()).await
  }

  async fn post_start(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self
      .underlying
      .post_start(context_handle.get_underlying().clone())
      .await
  }

  async fn pre_restart(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self
      .underlying
      .pre_restart(context_handle.get_underlying().clone())
      .await
  }

  async fn post_restart(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self
      .underlying
      .post_restart(context_handle.get_underlying().clone())
      .await
  }

  async fn pre_stop(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self.underlying.pre_stop(context_handle.get_underlying().clone()).await
  }

  async fn post_stop(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    self.underlying.post_stop(context_handle.get_underlying().clone()).await
  }

  async fn post_child_terminate(
    &mut self,
    context_handle: TypedContextHandle<M>,
    terminated: &Terminated,
  ) -> Result<(), ActorError> {
    self
      .underlying
      .post_child_terminate(context_handle.get_underlying().clone(), terminated)
      .await
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    self.underlying.get_supervisor_strategy().await
  }
}
