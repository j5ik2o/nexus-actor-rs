use std::fmt::Debug;

use log::error;
use crate::actor::actor::actor_error::ActorError;
use crate::actor::actor::actor_receive_func::ActorReceiveFunc;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::InfoPart;

#[derive(Debug, Clone)]
pub struct Behavior {
  stack: Vec<ActorReceiveFunc>,
}

impl Behavior {
  pub fn new() -> Self {
    Behavior { stack: vec![] }
  }

  pub async fn context_become(&mut self, receive: ActorReceiveFunc) {
    self.clear().await;
    self.push(receive).await;
  }

  pub async fn context_become_stacked(&mut self, receive: ActorReceiveFunc) {
    self.push(receive).await;
  }

  pub async fn context_un_become_stacked(&mut self) {
    self.pop().await;
  }

  pub async fn receive(&self, context: ContextHandle) -> Result<(), ActorError> {
    if let Some(behavior) = self.peek().await {
      behavior.run(context).await
    } else {
      error!("empty behavior called: pid = {}", context.get_self().await.unwrap());
      Err(ActorError::ReceiveError("empty behavior called".into()))
    }
  }

  async fn clear(&mut self) {
    for i in 0..self.stack.len() {
      self.stack[i] = ActorReceiveFunc::new(|_| async { Ok(()) });
    }
    self.stack.clear();
  }

  async fn peek(&self) -> Option<ActorReceiveFunc> {
    if let Some(last) = self.stack.last() {
      Some(last.clone())
    } else {
      None
    }
  }

  async fn push(&mut self, v: ActorReceiveFunc) {
    self.stack.push(v);
  }

  async fn pop(&mut self) -> Option<ActorReceiveFunc> {
    self.stack.pop()
  }

  pub(crate) fn len(&self) -> usize {
    self.stack.len()
  }
}
