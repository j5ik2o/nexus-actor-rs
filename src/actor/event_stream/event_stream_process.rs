use std::any::Any;

use async_trait::async_trait;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::MessageHandle;
use crate::actor::message_envelope::unwrap_envelope;
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
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message: MessageHandle) {
    let (_, msg, _) = unwrap_envelope(message);
    self.system.get_event_stream().await.publish(msg).await;
  }

  async fn send_system_message(&self, pid: &ExtendedPid, message: MessageHandle) {}

  async fn stop(&self, pid: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}
