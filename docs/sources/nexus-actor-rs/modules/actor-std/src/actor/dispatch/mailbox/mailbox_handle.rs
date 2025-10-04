use std::sync::Arc;

use async_trait::async_trait;

use crate::runtime::StdAsyncRwLock;

use crate::actor::dispatch::dispatcher::DispatcherHandle;
use crate::actor::dispatch::mailbox::{Mailbox, MailboxSyncHandle};
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::message::MessageHandle;
use nexus_actor_core_rs::actor::core_types::mailbox::{CoreMailbox, CoreMailboxFuture};
use nexus_actor_core_rs::runtime::{AsyncRwLock, AsyncYield};

#[derive(Debug, Clone)]
pub struct MailboxHandle {
  inner: Arc<StdAsyncRwLock<Box<dyn Mailbox>>>,
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
    (self.inner.as_ref() as *const StdAsyncRwLock<Box<dyn Mailbox>>).hash(state);
  }
}

impl MailboxHandle {
  pub fn new_arc(mailbox: Arc<StdAsyncRwLock<Box<dyn Mailbox>>>) -> Self {
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
      inner: Arc::new(StdAsyncRwLock::new(Box::new(mailbox) as Box<dyn Mailbox>)),
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

  async fn install_async_yielder(&mut self, yielder: Option<Arc<dyn AsyncYield>>) {
    let mut mg = self.inner.write().await;
    mg.install_async_yielder(yielder).await;
  }
}

impl CoreMailbox for MailboxHandle {
  fn post_user_message<'a>(&'a self, message: MessageHandle) -> CoreMailboxFuture<'a, ()> {
    Box::pin(async move { Mailbox::post_user_message(self, message).await })
  }

  fn post_system_message<'a>(&'a self, message: MessageHandle) -> CoreMailboxFuture<'a, ()> {
    Box::pin(async move { Mailbox::post_system_message(self, message).await })
  }

  fn process_messages<'a>(&'a self) -> CoreMailboxFuture<'a, ()> {
    Box::pin(async move { Mailbox::process_messages(self).await })
  }

  fn start<'a>(&'a self) -> CoreMailboxFuture<'a, ()> {
    Box::pin(async move { Mailbox::start(self).await })
  }

  fn user_messages_count<'a>(&'a self) -> CoreMailboxFuture<'a, i32> {
    Box::pin(async move { Mailbox::get_user_messages_count(self).await })
  }

  fn system_messages_count<'a>(&'a self) -> CoreMailboxFuture<'a, i32> {
    Box::pin(async move { Mailbox::get_system_messages_count(self).await })
  }
}
