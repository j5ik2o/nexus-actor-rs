use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::dispatch::dispatcher::DispatcherHandle;
use crate::actor::dispatch::mailbox::{Mailbox, MailboxSyncHandle};
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::message::MessageHandle;

#[derive(Debug, Clone)]
pub struct MailboxHandle {
  inner: Arc<RwLock<dyn Mailbox>>,
  sync: Option<MailboxSyncHandle>,
}

impl PartialEq for MailboxHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Eq for MailboxHandle {}

impl std::hash::Hash for MailboxHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.inner.as_ref() as *const RwLock<dyn Mailbox>).hash(state);
  }
}

impl MailboxHandle {
  pub fn new_arc(mailbox: Arc<RwLock<dyn Mailbox>>) -> Self {
    MailboxHandle {
      inner: mailbox,
      sync: None,
    }
  }

  pub fn new(mailbox: impl Mailbox + 'static) -> Self {
    Self::new_with_sync(mailbox, None)
  }

  pub fn new_with_sync(mailbox: impl Mailbox + 'static, sync: Option<MailboxSyncHandle>) -> Self {
    MailboxHandle {
      inner: Arc::new(RwLock::new(mailbox)),
      sync,
    }
  }

  pub fn sync_handle(&self) -> Option<MailboxSyncHandle> {
    self.sync.clone()
  }
}

#[async_trait]
impl Mailbox for MailboxHandle {
  async fn get_user_messages_count(&self) -> i32 {
    let mg = self.inner.read().await;
    mg.get_user_messages_count().await
  }

  async fn get_system_messages_count(&self) -> i32 {
    let mg = self.inner.read().await;
    mg.get_system_messages_count().await
  }

  async fn process_messages(&self) {
    let mg = self.inner.read().await;
    mg.process_messages().await;
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    let mg = self.inner.read().await;
    mg.post_user_message(message_handle).await;
  }

  async fn post_system_message(&self, message_handle: MessageHandle) {
    let mg = self.inner.read().await;
    mg.post_system_message(message_handle).await;
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    let mut mg = self.inner.write().await;
    mg.register_handlers(message_invoker_handle, dispatcher_handle).await;
  }

  async fn start(&self) {
    let mg = self.inner.read().await;
    mg.start().await;
  }

  async fn user_message_count(&self) -> i32 {
    let mg = self.inner.read().await;
    mg.user_message_count().await
  }

  async fn to_handle(&self) -> MailboxHandle {
    let mg = self.inner.read().await;
    mg.to_handle().await
  }
}
