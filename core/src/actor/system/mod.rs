//! Actor system module provides the core actor system functionality.

use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Message, MessageHandle, MessageOrEnvelope, Pid, Props, SpawnError};

#[derive(Debug)]
pub struct ActorSystem {
  inner: Arc<RwLock<dyn Debug + Send + Sync>>,
}

impl ActorSystem {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(RwLock::new(())),
    }
  }

  pub async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    // Implementation will be added later
    unimplemented!()
  }

  pub async fn send(&self, target: &Pid, message: MessageHandle) {
    // Implementation will be added later
    unimplemented!()
  }

  pub async fn stop(&self, pid: &Pid) {
    // Implementation will be added later
    unimplemented!()
  }
}

impl Default for ActorSystem {
  fn default() -> Self {
    Self::new()
  }
}
