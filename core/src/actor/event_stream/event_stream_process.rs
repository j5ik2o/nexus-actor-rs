use std::any::Any;

use async_trait::async_trait;

use crate::actor::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::message_or_envelope::unwrap_envelope_message;
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
    let msg_handle = unwrap_envelope_message(message_handle);
    let msg = msg_handle.as_any();
    self.system.get_event_stream().await.publish(msg).await;
  }

  async fn send_system_message(&self, _: &ExtendedPid, _: MessageHandle) {}

  async fn stop(&self, _: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}
