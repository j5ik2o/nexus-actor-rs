//! Actor system module provides core actor system functionality.

use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Message, MessageHandle, Pid, Props, SpawnError};
use crate::event_stream::EventStream;

#[derive(Debug)]
pub struct ActorSystem {
  event_stream: Arc<RwLock<EventStream>>,
}

impl ActorSystem {
  pub fn new() -> Self {
    Self {
      event_stream: Arc::new(RwLock::new(EventStream::new())),
    }
  }

  pub async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    // Implementation
    todo!()
  }

  pub async fn send(&self, target: &Pid, message: MessageHandle) {
    // Implementation
    todo!()
  }

  pub async fn stop(&self, pid: &Pid) {
    // Implementation
    todo!()
  }

  pub async fn event_stream(&self) -> Arc<RwLock<EventStream>> {
    self.event_stream.clone()
  }
}
