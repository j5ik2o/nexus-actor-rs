use std::sync::Arc;

use nexus_actor_core_rs::{FailureEvent, FailureEventListener};
use tokio::sync::broadcast;

/// Tokio broadcast ベースで FailureEvent を配信するラッパ。
#[derive(Clone)]
pub struct TokioFailureEventBridge {
  sender: broadcast::Sender<FailureEvent>,
}

impl TokioFailureEventBridge {
  pub fn new(capacity: usize) -> Self {
    let (sender, _) = broadcast::channel(capacity);
    Self { sender }
  }

  pub fn listener(&self) -> FailureEventListener {
    let sender = self.sender.clone();
    Arc::new(move |event: FailureEvent| {
      let _ = sender.send(event.clone());
    })
  }

  pub fn subscribe(&self) -> broadcast::Receiver<FailureEvent> {
    self.sender.subscribe()
  }
}
