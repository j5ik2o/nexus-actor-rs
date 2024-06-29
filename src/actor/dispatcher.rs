use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::Mutex;

use crate::actor::message::{Message, MessageHandle};
use crate::actor::{Reason, ReasonHandle};

pub struct Runnable(Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send + 'static>);

impl Runnable {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Runnable(Box::new(move || Box::pin(f()) as BoxFuture<'static, ()>))
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
    Self(dispatcher)
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

// --- TokioRuntimeContextDispatcher implementation

#[derive(Debug, Clone)]
pub struct TokioRuntimeContextDispatcher {
  throughput: i32,
}

impl TokioRuntimeContextDispatcher {
  pub fn new(throughput: i32) -> Self {
    Self { throughput }
  }
}

#[async_trait]
impl Dispatcher for TokioRuntimeContextDispatcher {
  async fn schedule(&self, runner: Runnable) {
    tokio::spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}

// --- TokioRuntimeDispatcher implementation

#[derive(Debug, Clone)]
pub struct TokioRuntimeDispatcher {
  runtime: Arc<Runtime>,
  throughput: i32,
}

impl TokioRuntimeDispatcher {
  pub fn new(runtime: Runtime, throughput: i32) -> Self {
    Self {
      runtime: Arc::new(runtime),
      throughput,
    }
  }
}

#[async_trait]
impl Dispatcher for TokioRuntimeDispatcher {
  async fn schedule(&self, runner: Runnable) {
    self.runtime.spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}

// --- SingleWorkerDispatcher implementation

#[derive(Debug, Clone)]
pub struct SingleWorkerDispatcher {
  runtime: Arc<Runtime>,
}

impl SingleWorkerDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    let runtime = Builder::new_multi_thread().worker_threads(1).enable_all().build()?;
    Ok(Self {
      runtime: Arc::new(runtime),
    })
  }
}

#[async_trait]
impl Dispatcher for SingleWorkerDispatcher {
  async fn schedule(&self, runner: Runnable) {
    self.runtime.spawn(async move {
      runner.run().await;
    });
  }

  async fn throughput(&self) -> i32 {
    1
  }
}

// --- CurrentThreadDispatcher implementation

#[derive(Debug, Clone)]
pub struct CurrentThreadDispatcher {
  runtime: Arc<Runtime>,
  throughput: i32,
}

impl CurrentThreadDispatcher {
  pub fn new(throughput: i32) -> Result<Self, std::io::Error> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    Ok(Self {
      runtime: Arc::new(runtime),
      throughput,
    })
  }
}

#[async_trait]
impl Dispatcher for CurrentThreadDispatcher {
  async fn schedule(&self, runner: Runnable) {
    self.runtime.block_on(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}

// Tests
#[cfg(test)]
mod tests {
  use crate::actor::mailbox::{DefaultMailbox, Mailbox};
  use crate::actor::message::MessageHandle;
  use crate::actor::message_invoker::{MessageInvoker, MessageInvokerHandle};
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
    let dispatcher = Arc::new(TokioRuntimeContextDispatcher::new(5));
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
