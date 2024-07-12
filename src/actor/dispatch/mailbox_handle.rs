use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::dispatch::dispatcher::DispatcherHandle;
use crate::actor::dispatch::mailbox::Mailbox;
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::message::message_handle::MessageHandle;

#[derive(Debug, Clone)]
pub struct MailboxHandle(Arc<Mutex<dyn Mailbox>>);

impl PartialEq for MailboxHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MailboxHandle {}

impl std::hash::Hash for MailboxHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn Mailbox>).hash(state);
  }
}

impl MailboxHandle {
  pub fn new_arc(mailbox: Arc<Mutex<dyn Mailbox>>) -> Self {
    MailboxHandle(mailbox)
  }

  pub fn new(mailbox: impl Mailbox + 'static) -> Self {
    MailboxHandle(Arc::new(Mutex::new(mailbox)))
  }
}

#[async_trait]
impl Mailbox for MailboxHandle {
  async fn process_messages(&self) {
    let mg = self.0.lock().await;
    mg.process_messages().await;
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    let mg = self.0.lock().await;
    mg.post_user_message(message_handle).await;
  }

  async fn post_system_message(&self, message_handle: MessageHandle) {
    let mg = self.0.lock().await;
    mg.post_system_message(message_handle).await;
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    let mut mg = self.0.lock().await;
    mg.register_handlers(message_invoker_handle, dispatcher_handle).await;
  }

  async fn start(&self) {
    let mg = self.0.lock().await;
    mg.start().await;
  }

  async fn user_message_count(&self) -> i32 {
    let mg = self.0.lock().await;
    mg.user_message_count().await
  }

  async fn to_handle(&self) -> MailboxHandle {
    let mg = self.0.lock().await;
    mg.to_handle().await
  }
}
