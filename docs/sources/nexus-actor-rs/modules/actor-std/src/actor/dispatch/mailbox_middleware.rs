use std::fmt::Debug;
use std::sync::Arc;

use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use tokio::sync::RwLock;

// MailboxMiddleware trait
#[async_trait]
pub trait MailboxMiddleware: Debug + Send + Sync {
  async fn mailbox_started(&mut self);
  async fn message_posted(&mut self, message_handle: &MessageHandle);
  async fn message_received(&mut self, message_handle: &MessageHandle);
  async fn mailbox_empty(&mut self);
}

#[derive(Debug, Clone)]
pub struct MailboxMiddlewareHandle(Arc<RwLock<dyn MailboxMiddleware>>);

impl PartialEq for MailboxMiddlewareHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MailboxMiddlewareHandle {}

impl std::hash::Hash for MailboxMiddlewareHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const RwLock<dyn MailboxMiddleware>).hash(state);
  }
}

impl MailboxMiddlewareHandle {
  pub fn new(middleware: impl MailboxMiddleware + 'static) -> Self {
    MailboxMiddlewareHandle(Arc::new(RwLock::new(middleware)))
  }
}

#[async_trait]
impl MailboxMiddleware for MailboxMiddlewareHandle {
  async fn mailbox_started(&mut self) {
    let mut mg = self.0.write().await;
    mg.mailbox_started().await;
  }

  async fn message_posted(&mut self, message_handle: &MessageHandle) {
    let mut mg = self.0.write().await;
    mg.message_posted(message_handle).await;
  }

  async fn message_received(&mut self, message_handle: &MessageHandle) {
    let mut mg = self.0.write().await;
    mg.message_received(message_handle).await;
  }

  async fn mailbox_empty(&mut self) {
    let mut mg = self.0.write().await;
    mg.mailbox_empty().await;
  }
}
