use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::dispatch::mailbox::{Mailbox, MailboxHandle};
use crate::actor::message::{Message, MessageHandle};
use crate::actor::actor::pid::ExtendedPid;
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
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message: MessageHandle) {
    self.mailbox.post_user_message(message).await;
  }

  async fn send_system_message(&self, _: &ExtendedPid, message: MessageHandle) {
    self.mailbox.post_system_message(message).await;
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self.set_dead();
    let stop_message = MessageHandle::new(StopMessage);
    self.send_system_message(pid, stop_message).await;
  }

  fn set_dead(&self) {
    self.dead.store(true, Ordering::SeqCst);
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[derive(Debug, Clone)]
struct StopMessage;

impl Message for StopMessage {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
