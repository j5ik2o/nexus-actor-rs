use async_trait::async_trait;
use std::fmt::Debug;
use std::time::Duration;

use crate::actor::{ActorError, Message, MessageHandle, Pid, SupervisorStrategy, SupervisorStrategyHandle};

#[derive(Debug, Clone)]
pub struct OneForOneStrategy {
  max_retries: i32,
  within_time: Duration,
}

#[async_trait]
impl SupervisorStrategy for OneForOneStrategy {
  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) -> bool {
    // Implement one-for-one restart strategy
    true // Always restart for now
  }
}

impl OneForOneStrategy {
  pub fn new(max_retries: i32, within_time: Duration) -> Self {
    Self {
      max_retries,
      within_time,
    }
  }

  pub fn into_handle(self) -> SupervisorStrategyHandle {
    SupervisorStrategyHandle::new(self)
  }
}
