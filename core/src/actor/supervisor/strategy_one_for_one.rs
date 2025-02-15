use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::time::Duration;

use crate::actor::{
  ActorError, ActorSystem, ErrorReason, ExtendedPid, Message, MessageHandle, Pid, RestartStatistics, SupervisorHandle,
  SupervisorStrategy,
};

#[derive(Debug, Clone)]
pub struct OneForOneStrategy {
  max_retries: i32,
  within_time: Duration,
}

#[async_trait]
impl SupervisorStrategy for OneForOneStrategy {
  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) -> bool {
    true // Always restart for now
  }

  async fn handle_child_failure(
    &self,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ErrorReason,
    message_handle: MessageHandle,
  ) {
    // Default implementation restarts the child
    actor_system.restart(&child.into()).await;
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl OneForOneStrategy {
  pub fn new(max_retries: i32, within_time: Duration) -> Self {
    Self {
      max_retries,
      within_time,
    }
  }

  pub fn into_handle(self) -> SupervisorHandle {
    SupervisorHandle::new(self)
  }
}
