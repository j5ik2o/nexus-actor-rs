//! Actor system module provides core actor system functionality.

use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::event_stream::EventStream;
use crate::actor::process_registry::ProcessRegistry;
use crate::actor::{Message, MessageHandle, Pid, Props, SpawnError};

#[derive(Debug)]
pub struct ActorSystem {
  event_stream: Arc<RwLock<EventStream>>,
  process_registry: Arc<RwLock<ProcessRegistry>>,
}

impl ActorSystem {
  pub fn new() -> Self {
    Self {
      event_stream: Arc::new(RwLock::new(EventStream::new())),
      process_registry: Arc::new(RwLock::new(ProcessRegistry::new())),
    }
  }

  pub async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    let pid = Pid::new("local".to_string(), 0); // TODO: Generate proper PID
    Ok(pid)
  }

  pub async fn send(&self, target: &Pid, message: MessageHandle) {
    // TODO: Implement message sending
  }

  pub async fn stop(&self, pid: &Pid) {
    // TODO: Implement actor stopping
  }

  pub async fn event_stream(&self) -> Arc<RwLock<EventStream>> {
    self.event_stream.clone()
  }

  pub async fn process_registry(&self) -> Arc<RwLock<ProcessRegistry>> {
    self.process_registry.clone()
  }
}

impl Default for ActorSystem {
  fn default() -> Self {
    Self::new()
  }
}
