use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::dispatch::dispatcher::DispatcherHandle;
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::message::MessageHandle;
use nexus_actor_core_rs::actor::core_types::mailbox::CoreMailboxQueue;
use nexus_actor_core_rs::runtime::AsyncYield;
use nexus_utils_std_rs::collections::QueueError;

pub use self::default_mailbox::{MailboxQueueLatencyMetrics, MailboxSuspensionMetrics};

#[derive(Debug, Clone)]
pub struct MailboxSyncHandle(Arc<dyn MailboxSync>);

impl PartialEq for MailboxSyncHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MailboxSyncHandle {}

impl std::hash::Hash for MailboxSyncHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn MailboxSync).hash(state);
  }
}

impl MailboxSyncHandle {
  pub fn new(mailbox: impl MailboxSync + 'static) -> Self {
    MailboxSyncHandle(Arc::new(mailbox))
  }

  pub fn user_messages_count(&self) -> i32 {
    self.0.user_messages_count()
  }

  pub fn system_messages_count(&self) -> i32 {
    self.0.system_messages_count()
  }

  pub fn suspension_metrics(&self) -> MailboxSuspensionMetrics {
    self.0.suspension_metrics()
  }

  pub fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics {
    self.0.queue_latency_metrics()
  }

  pub fn is_suspended(&self) -> bool {
    self.0.is_suspended()
  }

  pub fn core_queue_handles(
    &self,
  ) -> Option<(
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
  )> {
    self.0.core_queue_handles()
  }
}

pub trait MailboxSync: Debug + Send + Sync {
  fn user_messages_count(&self) -> i32;
  fn system_messages_count(&self) -> i32;
  fn suspension_metrics(&self) -> MailboxSuspensionMetrics;
  fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics;
  fn is_suspended(&self) -> bool;

  fn core_queue_handles(
    &self,
  ) -> Option<(
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
  )> {
    None
  }
}

impl MailboxSync for MailboxSyncHandle {
  fn user_messages_count(&self) -> i32 {
    self.user_messages_count()
  }

  fn system_messages_count(&self) -> i32 {
    self.system_messages_count()
  }

  fn suspension_metrics(&self) -> MailboxSuspensionMetrics {
    self.suspension_metrics()
  }

  fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics {
    self.queue_latency_metrics()
  }

  fn is_suspended(&self) -> bool {
    self.is_suspended()
  }

  fn core_queue_handles(
    &self,
  ) -> Option<(
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
  )> {
    self.core_queue_handles()
  }
}

pub(crate) mod core_queue_adapters;
mod default_mailbox;
mod mailbox_handle;
pub(crate) mod sync_queue_handles;
#[cfg(test)]
mod tests;

pub use default_mailbox::*;
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

  async fn install_async_yielder(&mut self, _yielder: Option<Arc<dyn AsyncYield>>) {}

  async fn to_handle(&self) -> MailboxHandle;
}
