use std::fmt::Debug;

use log::error;

use crate::actor::context::{ContextHandle, InfoPart};
use crate::actor::message::ReceiveFunc;

#[derive(Debug, Clone)]
pub struct Behavior {
  stack: Vec<ReceiveFunc>,
}

impl Behavior {
  pub fn new() -> Self {
    Behavior { stack: vec![] }
  }

  pub async fn context_become(&mut self, receive: ReceiveFunc) {
    self.clear().await;
    self.push(receive).await;
  }

  pub async fn context_become_stacked(&mut self, receive: ReceiveFunc) {
    self.push(receive).await;
  }

  pub async fn context_un_become_stacked(&mut self) {
    self.pop().await;
  }

  pub async fn receive(&self, context: ContextHandle) {
    if let Some(behavior) = self.peek().await {
      behavior.run(context).await;
    } else {
      error!("empty behavior called: pid = {}", context.get_self().await.unwrap());
    }
  }

  async fn clear(&mut self) {
    for i in 0..self.stack.len() {
      self.stack[i] = ReceiveFunc::new(|_| async {});
    }
    self.stack.clear();
  }

  async fn peek(&self) -> Option<ReceiveFunc> {
    if let Some(last) = self.stack.last() {
      Some(last.clone())
    } else {
      None
    }
  }

  async fn push(&mut self, v: ReceiveFunc) {
    self.stack.push(v);
  }

  async fn pop(&mut self) -> Option<ReceiveFunc> {
    self.stack.pop()
  }

  fn len(&self) -> usize {
    self.stack.len()
  }
}
