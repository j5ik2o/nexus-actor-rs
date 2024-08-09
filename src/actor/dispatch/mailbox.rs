use std::fmt::Debug;

use async_trait::async_trait;

use crate::actor::dispatch::dispatcher::DispatcherHandle;
use crate::actor::dispatch::mailbox_handle::MailboxHandle;
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::message::MessageHandle;

// Mailbox trait
#[async_trait]
pub trait Mailbox: Debug + Send + Sync {
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
