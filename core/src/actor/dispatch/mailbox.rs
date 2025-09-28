use std::fmt::Debug;

use async_trait::async_trait;

use crate::actor::dispatch::dispatcher::DispatcherHandle;
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::message::MessageHandle;

mod default_mailbox;
mod mailbox_handle;
pub(crate) mod sync_queue_handles;
#[cfg(test)]
mod tests;

pub(crate) use default_mailbox::*;
pub use mailbox_handle::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MailboxQueueKind {
  User,
  System,
}

impl MailboxQueueKind {
  pub fn as_str(&self) -> &'static str {
    match self {
      MailboxQueueKind::User => "user",
      MailboxQueueKind::System => "system",
    }
  }
}

// Mailbox trait
#[async_trait]
pub trait Mailbox: Debug + Send + Sync {
  async fn get_user_messages_count(&self) -> i32;
  async fn get_system_messages_count(&self) -> i32;

  async fn process_messages(&self);
  async fn post_user_message(&self, message_handle: MessageHandle);
  async fn post_system_message(&self, message_handle: MessageHandle);
  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  );
  async fn start(&self);
  async fn user_message_count(&self) -> i32;

  async fn to_handle(&self) -> MailboxHandle;
}
