use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::core::pid::ExtendedPid;
use crate::actor::dispatch::{Mailbox, MailboxHandle};
use crate::actor::message::MessageHandle;
use crate::actor::message::SystemMessage;
use crate::actor::process::Process;

#[derive(Debug, Clone)]
pub struct ActorProcess {
  mailbox: MailboxHandle,
  dead: Arc<AtomicBool>,
}

impl PartialEq for ActorProcess {
  fn eq(&self, other: &Self) -> bool {
    self.mailbox == other.mailbox && Arc::ptr_eq(&self.dead, &other.dead)
  }
}

impl Eq for ActorProcess {}

impl std::hash::Hash for ActorProcess {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.mailbox.hash(state);
    (self.dead.as_ref() as *const AtomicBool).hash(state);
  }
}

impl ActorProcess {
  pub fn new(mailbox: MailboxHandle) -> Self {
    Self {
      mailbox,
      dead: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn is_dead(&self) -> bool {
    self.dead.load(Ordering::SeqCst)
  }

  pub fn mailbox_handle(&self) -> MailboxHandle {
    self.mailbox.clone()
  }
}

#[async_trait]
impl Process for ActorProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message_handle: MessageHandle) {
    tracing::debug!("ActorProcess::send_user_message: {:?}", message_handle);
    self.mailbox.post_user_message(message_handle).await;
  }

  async fn send_system_message(&self, _: &ExtendedPid, message_handle: MessageHandle) {
    tracing::debug!("ActorProcess::send_system_message: {:?}", message_handle);
    self.mailbox.post_system_message(message_handle).await;
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self.set_dead();
    self
      .send_system_message(pid, MessageHandle::new(SystemMessage::Stop))
      .await;
  }

  fn set_dead(&self) {
    self.dead.store(true, Ordering::SeqCst);
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
