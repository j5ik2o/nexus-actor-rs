use crate::actor::actor::{Actor, ActorError, ActorReceiver};
use crate::actor::context::{ContextHandle, InfoPart};
use crate::actor::util::stack::Stack;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ActorBehavior {
  stack: Arc<Mutex<Stack<ActorReceiver>>>,
}

impl ActorBehavior {
  pub fn new() -> Self {
    Self {
      stack: Arc::new(Mutex::new(Stack::new())),
    }
  }

  pub async fn reset(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.lock().await;
    mg.clear();
    mg.push(receiver);
  }

  pub async fn become_stacked(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.lock().await;
    mg.push(receiver);
  }

  pub async fn un_become_stacked(&mut self) {
    let mut mg = self.stack.lock().await;
    mg.pop();
  }

  pub async fn clear(&mut self) {
    let mut mg = self.stack.lock().await;
    mg.clear();
  }
}

#[async_trait]
impl Actor for ActorBehavior {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    if let Some(behavior) = {
      let mg = self.stack.lock().await;
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
