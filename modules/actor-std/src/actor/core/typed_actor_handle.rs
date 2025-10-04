use crate::actor::context::TypedContextHandle;
use crate::actor::core::typed_actor::TypedActor;
use crate::actor::core::{Actor, ActorError, ActorHandle};
use crate::actor::message::{Message, TerminatedMessage};
use crate::actor::supervisor::SupervisorStrategyHandle;
use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct TypedActorHandle<M: Message> {
  inner: Arc<RwLock<dyn TypedActor<M>>>,
}

#[derive(Debug, Clone)]
pub struct WeakTypedActorHandle<M: Message> {
  inner: Weak<RwLock<dyn TypedActor<M>>>,
}

impl<M: Message> TypedActorHandle<M> {
  pub fn downgrade(&self) -> WeakTypedActorHandle<M> {
    WeakTypedActorHandle {
      inner: Arc::downgrade(&self.inner),
    }
  }
}

impl<M: Message> WeakTypedActorHandle<M> {
  pub fn upgrade(&self) -> Option<TypedActorHandle<M>> {
    self.inner.upgrade().map(|inner| TypedActorHandle { inner })
  }

  pub fn is_alive(&self) -> bool {
    self.inner.strong_count() > 0
  }
}

impl<M: Message> PartialEq for TypedActorHandle<M> {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl<M: Message> Eq for TypedActorHandle<M> {}

impl<M: Message> std::hash::Hash for TypedActorHandle<M> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.inner.as_ref() as *const RwLock<dyn TypedActor<M>>).hash(state);
  }
}

impl<M: Message> PartialEq for WeakTypedActorHandle<M> {
  fn eq(&self, other: &Self) -> bool {
    Weak::ptr_eq(&self.inner, &other.inner)
  }
}

impl<M: Message> Eq for WeakTypedActorHandle<M> {}

impl<M: Message> std::hash::Hash for WeakTypedActorHandle<M> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.inner.as_ptr() as *const RwLock<dyn TypedActor<M>>).hash(state);
  }
}

impl<M: Message + Clone> TypedActorHandle<M> {
  pub fn new_arc(actor: Arc<RwLock<dyn TypedActor<M>>>) -> Self {
    TypedActorHandle { inner: actor }
  }

  pub fn new(actor: impl TypedActor<M> + 'static) -> Self {
    let inner: Arc<RwLock<dyn TypedActor<M>>> = Arc::new(RwLock::new(actor));
    TypedActorHandle { inner }
  }
}

#[async_trait]
impl<M: Message + Clone> TypedActor<M> for TypedActorHandle<M> {
  async fn handle(&mut self, c: TypedContextHandle<M>) -> Result<(), ActorError> {
    let mut mg = self.inner.write().await;
    mg.handle(c).await
  }

  async fn receive(&mut self, context_handle: TypedContextHandle<M>) -> Result<(), ActorError> {
    let mut mg = self.inner.write().await;
    mg.receive(context_handle).await
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    let mut mg = self.inner.write().await;
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
    terminated: &TerminatedMessage,
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

#[cfg(test)]
mod tests {
  use super::*;
  use async_trait::async_trait;
  use nexus_message_derive_rs::Message;

  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  struct TestMessage;

  #[derive(Debug)]
  struct TestTypedActor;

  #[async_trait]
  impl TypedActor<TestMessage> for TestTypedActor {
    async fn receive(&mut self, _context_handle: TypedContextHandle<TestMessage>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[test]
  fn downgrade_and_upgrade_weak_handle() {
    let weak_handle;
    {
      let handle = TypedActorHandle::<TestMessage>::new(TestTypedActor);
      weak_handle = handle.downgrade();
      assert!(weak_handle.is_alive());
      assert!(weak_handle.upgrade().is_some());
    }
    assert!(!weak_handle.is_alive());
    assert!(weak_handle.upgrade().is_none());
  }
}
