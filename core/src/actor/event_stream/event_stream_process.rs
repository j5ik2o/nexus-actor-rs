use std::any::Any;

use async_trait::async_trait;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ExtendedPid;
use crate::actor::message::unwrap_envelope;
use crate::actor::message::MessageHandle;
use crate::actor::process::Process;

#[derive(Debug, Clone)]
pub struct EventStreamProcess {
  system: ActorSystem,
}

impl EventStreamProcess {
  pub fn new(system: ActorSystem) -> Self {
    EventStreamProcess { system }
  }
}

#[async_trait]
impl Process for EventStreamProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message_handle: MessageHandle) {
    let (_, msg, _) = unwrap_envelope(message_handle);
    self.system.get_event_stream().await.publish(msg).await;
  }

  async fn send_system_message(&self, _: &ExtendedPid, _: MessageHandle) {}

  async fn stop(&self, _: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[cfg(test)]
mod tests;
