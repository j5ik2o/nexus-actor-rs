use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::dispatch::mailbox::Mailbox;
use crate::actor::dispatch::mailbox_handle::MailboxHandle;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::system_message::SystemMessage;
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
}

#[async_trait]
impl Process for ActorProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message_handle: MessageHandle) {
    self.mailbox.post_user_message(message_handle).await;
  }

  async fn send_system_message(&self, _: &ExtendedPid, message_handle: MessageHandle) {
    self.mailbox.post_system_message(message_handle).await;
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self.set_dead();
    let stop_message = MessageHandle::new(SystemMessage::Stop);
    self.send_system_message(pid, stop_message).await;
  }

  fn set_dead(&self) {
    self.dead.store(true, Ordering::SeqCst);
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
