use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::any::Any;

// Message and Task traits
trait Message: Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
}

trait Task: Message {
  fn run(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

// MailboxMiddleware trait
#[async_trait]
trait MailboxMiddleware: Send + Sync {
  async fn mailbox_started(&self);
  async fn message_posted(&self, message: &dyn Message);
  async fn message_received(&self, message: &dyn Message);
  async fn mailbox_empty(&self);
}

// MessageInvoker trait
#[async_trait]
trait MessageInvoker: Send + Sync {
  async fn invoke_system_message(&self, message: &Box<dyn Message>);
  async fn invoke_user_message(&self, message: &Box<dyn Message>);
  async fn escalate_failure(&self, reason: Box<dyn Any + Send>, message: &Box<dyn Message>);
}

// Dispatcher trait
#[async_trait]
trait Dispatcher: Send + Sync {
  async fn schedule(&self, runner: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>);
  async fn throughput(&self) -> i32;
}

// Mailbox trait
#[async_trait]
trait Mailbox: Send + Sync {
  async fn post_user_message(self: &Arc<Self>, message: Box<dyn Message>);
  async fn post_system_message(self: &Arc<Self>, message: Box<dyn Message>);
  async fn register_handlers(self: &Arc<Self>, invoker: Option<Arc<dyn MessageInvoker>>, dispatcher: Option<Arc<dyn Dispatcher>>);
  async fn start(self: &Arc<Self>);
  fn user_message_count(self: &Arc<Self>) -> i32;
}

// DefaultMailbox implementation
struct DefaultMailbox {
  user_mailbox_sender: tokio::sync::mpsc::UnboundedSender<Box<dyn Message>>,
  user_mailbox_receiver: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<Box<dyn Message>>>>,
  system_mailbox: Arc<Mutex<Vec<Box<dyn Message>>>>,
  scheduler_status: AtomicBool,
  user_messages: AtomicI32,
  sys_messages: AtomicI32,
  suspended: AtomicBool,
  invoker_opt: Mutex<Option<Arc<dyn MessageInvoker>>>,
  dispatcher_opt: Mutex<Option<Arc<dyn Dispatcher>>>,
  middlewares: Vec<Arc<dyn MailboxMiddleware>>,
}

impl DefaultMailbox {
  fn new() -> Arc<Self> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    Arc::new(DefaultMailbox {
      user_mailbox_sender: sender,
      user_mailbox_receiver: Arc::new(Mutex::new(receiver)),
      system_mailbox: Arc::new(Mutex::new(Vec::new())),
      scheduler_status: AtomicBool::new(false),
      user_messages: AtomicI32::new(0),
      sys_messages: AtomicI32::new(0),
      suspended: AtomicBool::new(false),
      invoker_opt: Mutex::new(None),
      dispatcher_opt: Mutex::new(None),
      middlewares: Vec::new(),
    })
  }

  async fn schedule(self: &Arc<Self>) {
    if self.scheduler_status.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
      if let Some(dispatcher) = &*self.dispatcher_opt.lock().await  {
        let self_clone = self.clone();
        dispatcher.schedule(Box::new(move || {
          let self_clone = Arc::clone(&self_clone);
          Box::pin(async move {
            self_clone.process_messages().await;
          })
        })).await;
      }
    }
  }

  async fn process_messages(&self) {
    loop {
      self.run().await;

      self.scheduler_status.store(false, Ordering::SeqCst);
      let sys = self.sys_messages.load(Ordering::SeqCst);
      let user = self.user_messages.load(Ordering::SeqCst);

      if sys > 0 || (!self.suspended.load(Ordering::SeqCst) && user > 0) {
        if self.scheduler_status.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
          continue;
        }
      }

      break;
    }

    for middleware in &self.middlewares {
      middleware.mailbox_empty().await;
    }
  }

  async fn run(&self) {
    let mut i = 0;
    let dispatcher_lock = self.dispatcher_opt.lock().await;
    if dispatcher_lock.is_none() {
      return;
    }
    let invoker_lock = self.invoker_opt.lock().await;
    if invoker_lock.is_none() {
      return;
    }
    let dispatcher = dispatcher_lock.as_ref().unwrap();
    let invoker = invoker_lock.as_ref().unwrap();

    let t = dispatcher.throughput().await;

    loop {
      if i > t {
        i = 0;
        tokio::task::yield_now().await;
      }

      i += 1;

      if let Some(msg) = self.system_mailbox.lock().await.pop() {
        self.sys_messages.fetch_sub(1, Ordering::SeqCst);
        invoker.invoke_system_message(&msg).await;
        for middleware in &self.middlewares {
          middleware.message_received(msg.as_ref()).await;
        }
        continue;
      }

      if self.suspended.load(Ordering::SeqCst) {
        break;
      }

      if let Ok(msg) = self.user_mailbox_receiver.lock().await.try_recv() {
        self.user_messages.fetch_sub(1, Ordering::SeqCst);
        invoker.invoke_user_message(&msg).await;
        for middleware in &self.middlewares {
          middleware.message_received(msg.as_ref()).await;
        }
      } else {
        break;
      }
    }
  }
}

#[async_trait]
impl Mailbox for DefaultMailbox {
  async fn post_user_message(self: &Arc<Self>, message: Box<dyn Message>) {
    for middleware in &self.middlewares {
      middleware.message_posted(message.as_ref()).await;
    }

    if let Err(e) = self.user_mailbox_sender.send(message) {
      println!("Failed to send message: {:?}", e);
    } else {
      self.user_messages.fetch_add(1, Ordering::SeqCst);
      self.schedule().await;
    }
  }

  async fn post_system_message(self: &Arc<Self>, message: Box<dyn Message>) {
    for middleware in &self.middlewares {
      middleware.message_posted(message.as_ref()).await;
    }

    self.system_mailbox.lock().await.push(message);
    self.sys_messages.fetch_add(1, Ordering::SeqCst);
    self.schedule().await;
  }

  async fn register_handlers(self: &Arc<Self>,  invoker: Option<Arc<dyn MessageInvoker>>, dispatcher: Option<Arc<dyn Dispatcher>>) {
    *self.invoker_opt.lock().await = invoker;
    *self.dispatcher_opt.lock().await = dispatcher;
  }

  async fn start(self: &Arc<Self>) {
    for middleware in &self.middlewares {
      middleware.mailbox_started().await;
    }
  }

  fn user_message_count(self: &Arc<Self>) -> i32 {
    self.user_messages.load(Ordering::SeqCst)
  }
}

// TokioDispatcher implementation
struct TokioDispatcher {
  throughput: AtomicI32,
}

impl TokioDispatcher {
  fn new(throughput: i32) -> Self {
    TokioDispatcher {
      throughput: AtomicI32::new(throughput),
    }
  }
}

#[async_trait]
impl Dispatcher for TokioDispatcher {
  async fn schedule(&self, runner: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>) {
    tokio::spawn(async move {
      runner().await;
    });
  }

  async fn throughput(&self) -> i32 {
    self.throughput.load(Ordering::Relaxed)
  }
}

// TestMessageInvoker implementation
#[derive(Debug, Clone, PartialEq)]
enum ReceivedMessage {
  System,
  User,
  Task,
  Failure(String),
}

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
  async fn invoke_system_message(&self, _message: &Box<dyn Message>) {
    self.received.lock().await.push(ReceivedMessage::System);
  }

  async fn invoke_user_message(&self, message: &Box<dyn Message>) {
    if message.as_any().is::<TestTask>() {
      self.received.lock().await.push(ReceivedMessage::Task);
    } else {
      self.received.lock().await.push(ReceivedMessage::User);
    }
  }

  async fn escalate_failure(&self, reason: Box<dyn Any + Send>, _message: &Box<dyn Message>) {
    let reason_msg = if let Some(error) = reason.downcast_ref::<&str>() {
      error.to_string()
    } else if let Some(error) = reason.downcast_ref::<String>() {
      error.clone()
    } else {
      "Unknown error".to_string()
    };
    self.received.lock().await.push(ReceivedMessage::Failure(reason_msg));
  }
}

// Test message types
struct TestSystemMessage;
impl Message for TestSystemMessage {
  fn as_any(&self) -> &dyn Any { self }
}

struct TestUserMessage;
impl Message for TestUserMessage {
  fn as_any(&self) -> &dyn Any { self }
}

struct TestTask;
impl Message for TestTask {
  fn as_any(&self) -> &dyn Any { self }
}
impl Task for TestTask {
  fn run(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async {})
  }
}

// Tests
#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_mailbox_with_test_invoker() {
    let invoker = Arc::new(TestMessageInvoker::new());
    let dispatcher = Arc::new(TokioDispatcher::new(10));
    let mailbox = DefaultMailbox::new();
    mailbox.register_handlers(Some(invoker.clone()), Some(dispatcher.clone())).await;

    let system_message = Box::new(TestSystemMessage);
    let user_message = Box::new(TestUserMessage);
    let task = Box::new(TestTask);

    mailbox.post_system_message(system_message).await;
    mailbox.post_user_message(user_message).await;
    mailbox.post_user_message(task).await;

    // メッセージが処理されるのを少し待つ
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let received = invoker.get_received().await;
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], ReceivedMessage::System);
    assert_eq!(received[1], ReceivedMessage::User);
    assert_eq!(received[2], ReceivedMessage::Task);

  }
}