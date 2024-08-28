use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::message::MessageHandle;

// MailboxMiddleware trait
#[async_trait]
pub trait MailboxMiddleware: Debug + Send + Sync {
  async fn mailbox_started(&self);
  async fn message_posted(&self, message_handle: MessageHandle);
  async fn message_received(&self, message_handle: MessageHandle);
  async fn mailbox_empty(&self);
}

#[derive(Debug, Clone)]
pub struct MailboxMiddlewareHandle(Arc<dyn MailboxMiddleware>);

impl PartialEq for MailboxMiddlewareHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MailboxMiddlewareHandle {}

impl std::hash::Hash for MailboxMiddlewareHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn MailboxMiddleware).hash(state);
  }
}

impl MailboxMiddlewareHandle {
  pub fn new(middleware: impl MailboxMiddleware + 'static) -> Self {
    MailboxMiddlewareHandle(Arc::new(middleware))
  }
}

#[async_trait]
impl MailboxMiddleware for MailboxMiddlewareHandle {
  async fn mailbox_started(&self) {
    self.0.mailbox_started().await;
  }

  async fn message_posted(&self, message_handle: MessageHandle) {
    self.0.message_posted(message_handle).await;
  }

  async fn message_received(&self, message_handle: MessageHandle) {
    self.0.message_received(message_handle).await;
  }

  async fn mailbox_empty(&self) {
    self.0.mailbox_empty().await;
  }
}
