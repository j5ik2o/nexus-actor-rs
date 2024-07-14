use crate::actor::actor::actor_error::ActorError;
use crate::actor::actor::actor_receiver::ActorReceiver;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::InfoPart;
use std::fmt::Debug;
use std::future::Future;

#[derive(Debug, Clone)]
pub struct Behavior {
  stack: Vec<ActorReceiver>,
}

impl Behavior {
  pub fn new() -> Self {
    Behavior { stack: vec![] }
  }

  pub async fn transition<F, Fut>(&mut self, receive: F)
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    self.clear().await;
    self.push(receive).await;
  }

  pub async fn transition_stacked<F, Fut>(&mut self, receive: F)
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    self.push(receive).await;
  }

  pub async fn revert_transition(&mut self) {
    self.pop().await;
  }

  pub async fn receive(&self, context: ContextHandle) -> Result<(), ActorError> {
    if let Some(behavior) = self.peek().await {
      behavior.run(context).await
    } else {
      tracing::error!("empty behavior called: pid = {}", context.get_self().await);
      Err(ActorError::ReceiveError("empty behavior called".into()))
    }
  }

  pub(crate) async fn clear(&mut self) {
    for i in 0..self.stack.len() {
      self.stack[i] = ActorReceiver::new(|_| async { Ok(()) });
    }
    self.stack.clear();
  }

  pub(crate) async fn peek(&self) -> Option<ActorReceiver> {
    if let Some(last) = self.stack.last() {
      Some(last.clone())
    } else {
      None
    }
  }

  pub(crate) async fn push<F, Fut>(&mut self, actor_receiver: F)
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    self.push_actor_receiver(ActorReceiver::new(actor_receiver)).await;
  }

  pub(crate) async fn push_actor_receiver(&mut self, actor_receiver: ActorReceiver) {
    self.stack.push(actor_receiver);
  }

  pub(crate) async fn pop(&mut self) -> Option<ActorReceiver> {
    self.stack.pop()
  }

  pub(crate) fn len(&self) -> usize {
    self.stack.len()
  }
}
