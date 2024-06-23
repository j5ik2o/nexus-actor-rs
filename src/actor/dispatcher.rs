use std::any::Any;
use std::fmt::Debug;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::sync::Mutex;

use crate::actor::message::{Message, MessageHandle};
use crate::actor::{Reason, ReasonHandle};

// MailboxMiddleware trait
#[async_trait]
pub trait MailboxMiddleware: Debug + Send + Sync {
  async fn mailbox_started(&self);
  async fn message_posted(&self, message: MessageHandle);
  async fn message_received(&self, message: MessageHandle);
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
  pub fn new_arc(middleware: Arc<dyn MailboxMiddleware>) -> Self {
    MailboxMiddlewareHandle(middleware)
  }

  pub fn new(middleware: impl MailboxMiddleware + Send + Sync + 'static) -> Self {
    MailboxMiddlewareHandle(Arc::new(middleware))
  }
}

#[async_trait]
impl MailboxMiddleware for MailboxMiddlewareHandle {
  async fn mailbox_started(&self) {
    self.0.mailbox_started().await;
  }

  async fn message_posted(&self, message: MessageHandle) {
    self.0.message_posted(message).await;
  }

  async fn message_received(&self, message: MessageHandle) {
    self.0.message_received(message).await;
  }

  async fn mailbox_empty(&self) {
    self.0.mailbox_empty().await;
  }
}

// MessageInvoker trait
#[async_trait]
pub trait MessageInvoker: Debug + Send + Sync {
  async fn invoke_system_message(&mut self, message: MessageHandle);
  async fn invoke_user_message(&mut self, message: MessageHandle);
  async fn escalate_failure(&self, reason: ReasonHandle, message: MessageHandle);
}

#[derive(Debug, Clone)]
pub struct MessageInvokerHandle(Arc<Mutex<dyn MessageInvoker>>);

impl MessageInvokerHandle {
  pub fn new(invoker: Arc<Mutex<dyn MessageInvoker>>) -> Self {
    MessageInvokerHandle(invoker)
  }
}

impl PartialEq for MessageInvokerHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MessageInvokerHandle {}

impl std::hash::Hash for MessageInvokerHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn MessageInvoker>).hash(state);
  }
}

#[async_trait]
impl MessageInvoker for MessageInvokerHandle {
  async fn invoke_system_message(&mut self, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.invoke_system_message(message).await;
  }

  async fn invoke_user_message(&mut self, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.invoke_user_message(message).await;
  }

  async fn escalate_failure(&self, reason: ReasonHandle, message: MessageHandle) {
    let mg = self.0.lock().await;
    mg.escalate_failure(reason, message).await;
  }
}

pub struct Runnable(Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>);

impl Runnable {
  pub fn new<F>(f: F) -> Self
  where
    F: FnOnce() -> BoxFuture<'static, ()> + Send + 'static, {
    Runnable(Box::new(f))
  }

  pub fn run(self) -> BoxFuture<'static, ()> {
    (self.0)()
  }
}

// Dispatcher trait
#[async_trait]
pub trait Dispatcher: Debug + Send + Sync + 'static {
  async fn schedule(&self, runner: Runnable);
  async fn throughput(&self) -> i32;
}

#[derive(Debug, Clone)]
pub struct DispatcherHandle(Arc<dyn Dispatcher>);

impl DispatcherHandle {
  pub fn new(dispatcher: Arc<dyn Dispatcher>) -> Self {
    DispatcherHandle(dispatcher)
  }
}

#[async_trait]
impl Dispatcher for DispatcherHandle {
  async fn schedule(&self, runner: Runnable) {
    self.0.schedule(runner).await;
  }

  async fn throughput(&self) -> i32 {
    self.0.throughput().await
  }
}

// TokioDispatcher implementation
#[derive(Debug, Clone)]
pub struct TokioDispatcher {
  throughput: Arc<AtomicI32>,
}

impl TokioDispatcher {
  pub fn new(throughput: i32) -> Self {
    TokioDispatcher {
      throughput: Arc::new(AtomicI32::new(throughput)),
    }
  }
}

#[async_trait]
impl Dispatcher for TokioDispatcher {
  async fn schedule(&self, runner: Runnable) {
    tokio::spawn(async move {
      runner.run().await;
    });
  }

  async fn throughput(&self) -> i32 {
    self.throughput.load(Ordering::Relaxed)
  }
}

// Tests
#[cfg(test)]
mod tests {
  use crate::actor::mailbox::{DefaultMailbox, Mailbox};
  use crate::actor::message::MessageHandle;
  use crate::actor::taks::Task;
  use crate::util::queue::mpsc_unbounded_channel_queue::MpscUnboundedChannelQueue;

  use super::*;

  // TestMessageInvoker implementation
  #[derive(Debug, Clone, PartialEq)]
  enum ReceivedMessage {
    System,
    User,
    Task,
    Failure(String),
  }

  #[derive(Debug)]
  struct TestMessageInvoker {
    received: Arc<Mutex<Vec<ReceivedMessage>>>,
  }

  impl TestMessageInvoker {
    fn new() -> Self {
      TestMessageInvoker {
        received: Arc::new(Mutex::new(Vec::new())),
      }
    }

    async fn get_received(&self) -> Vec<ReceivedMessage> {
      self.received.lock().await.clone()
    }
  }

  #[async_trait]
  impl MessageInvoker for TestMessageInvoker {
    async fn invoke_system_message(&mut self, _message: MessageHandle) {
      self.received.lock().await.push(ReceivedMessage::System);
    }

    async fn invoke_user_message(&mut self, message: MessageHandle) {
      if message.as_any().is::<TestTask>() {
        self.received.lock().await.push(ReceivedMessage::Task);
      } else {
        self.received.lock().await.push(ReceivedMessage::User);
      }
    }

    async fn escalate_failure(&self, reason: ReasonHandle, _message: MessageHandle) {
      let reason_msg = if let Some(error) = reason.as_any().downcast_ref::<&str>() {
        error.to_string()
      } else if let Some(error) = reason.as_any().downcast_ref::<String>() {
        error.clone()
      } else {
        "Unknown error".to_string()
      };
      self.received.lock().await.push(ReceivedMessage::Failure(reason_msg));
    }
  }

  // Test message types
  #[derive(Debug, Clone)]
  struct TestSystemMessage;

  impl Message for TestSystemMessage {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone)]
  struct TestUserMessage;

  impl Message for TestUserMessage {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone)]
  struct TestTask;

  impl Message for TestTask {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[async_trait]
  impl Task for TestTask {
    async fn run(&self) {}
  }

  #[tokio::test]
  async fn test_mailbox_with_test_invoker() {
    let mut mailbox = DefaultMailbox::new(MpscUnboundedChannelQueue::new(), MpscUnboundedChannelQueue::new());
    let invoker = Arc::new(Mutex::new(TestMessageInvoker::new()));
    let invoker_handle = MessageInvokerHandle::new(invoker.clone());
    let dispatcher = Arc::new(TokioDispatcher::new(10));
    let dispatcher_handle = DispatcherHandle::new(dispatcher.clone());
    mailbox
      .register_handlers(Some(invoker_handle.clone()), Some(dispatcher_handle.clone()))
      .await;

    let system_message = MessageHandle::new(TestSystemMessage);
    let user_message = MessageHandle::new(TestUserMessage);
    let task = MessageHandle::new(TestTask);

    mailbox.post_system_message(system_message).await;
    mailbox.post_user_message(user_message).await;
    mailbox.post_user_message(task).await;

    // メッセージが処理されるのを少し待つ
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let invoker_mg = invoker.lock().await;
    let received = invoker_mg.get_received().await;
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], ReceivedMessage::System);
    assert_eq!(received[1], ReceivedMessage::User);
    assert_eq!(received[2], ReceivedMessage::Task);
  }
}
