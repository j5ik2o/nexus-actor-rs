#[cfg(test)]
mod tests;

use crate::actor::context::{ContextHandle, InfoPart};
use crate::actor::core::{Actor, ActorError, ActorReceiver};
use async_trait::async_trait;
use nexus_utils_std_rs::collections::Stack;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct ActorBehavior {
  stack: Arc<RwLock<Stack<ActorReceiver>>>,
}

impl ActorBehavior {
  pub fn new() -> Self {
    Self {
      stack: Arc::new(RwLock::new(Stack::new())),
    }
  }

  pub async fn reset(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.write().await;
    mg.clear();
    mg.push(receiver);
  }

  pub async fn become_stacked(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.write().await;
    mg.push(receiver);
  }

  pub async fn un_become_stacked(&mut self) {
    let mut mg = self.stack.write().await;
    mg.pop();
  }

  pub async fn clear(&mut self) {
    let mut mg = self.stack.write().await;
    mg.clear();
  }
}

impl Default for ActorBehavior {
  fn default() -> Self {
    ActorBehavior::new()
  }
}

#[async_trait]
impl Actor for ActorBehavior {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    if let Some(behavior) = {
      let mg = self.stack.read().await;
      mg.peek()
    } {
      behavior.run(context_handle.clone()).await?;
    } else {
      tracing::error!("empty behavior called: pid = {}", context_handle.get_self().await);
    }
    Ok(())
  }
}

static_assertions::assert_impl_all!(ActorBehavior: Send, Sync);
