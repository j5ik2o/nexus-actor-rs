use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Message, MessageHandle, MessageOrEnvelope, Pid, Props, SpawnError};

#[derive(Debug)]
pub struct ActorSystem {
  // Implementation details to be added
}

impl ActorSystem {
  pub fn new() -> Self {
    Self {}
  }
}
