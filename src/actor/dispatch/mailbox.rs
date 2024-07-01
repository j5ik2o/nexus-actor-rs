use std::fmt::Debug;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::sync::Mutex;

use crate::actor::dispatch::dispatcher::{Dispatcher, DispatcherHandle, Runnable};
use crate::actor::dispatch::mailbox_middleware::{MailboxMiddleware, MailboxMiddlewareHandle};
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::message::{Message, MessageHandle};
use crate::actor::messages::MailboxMessage;
use crate::util::queue::{QueueError, QueueReader, QueueWriter};

// Mailbox trait
#[async_trait]
pub trait Mailbox: Debug + Send + Sync {
  async fn process_messages(&self);
  async fn post_user_message(&self, message: MessageHandle);
  async fn post_system_message(&self, message: MessageHandle);
  async fn register_handlers(&mut self, invoker: Option<MessageInvokerHandle>, dispatcher: Option<DispatcherHandle>);
  async fn start(&self);
  async fn user_message_count(&self) -> i32;

  async fn to_handle(&self) -> MailboxHandle;
}

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

  async fn post_user_message(&self, message: MessageHandle) {
    let mg = self.0.lock().await;
    mg.post_user_message(message).await;
  }

  async fn post_system_message(&self, message: MessageHandle) {
    let mg = self.0.lock().await;
    mg.post_system_message(message).await;
  }

  async fn register_handlers(&mut self, invoker: Option<MessageInvokerHandle>, dispatcher: Option<DispatcherHandle>) {
    let mut mg = self.0.lock().await;
    mg.register_handlers(invoker, dispatcher).await;
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

#[derive(Clone)]
pub struct MailboxProduceFunc(Arc<dyn Fn() -> BoxFuture<'static, MailboxHandle> + Send + Sync>);

impl Debug for MailboxProduceFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "MailboxProducer")
  }
}

impl PartialEq for MailboxProduceFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for MailboxProduceFunc {}

impl std::hash::Hash for MailboxProduceFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn() -> BoxFuture<'static, MailboxHandle>).hash(state);
  }
}

impl MailboxProduceFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MailboxHandle> + Send + 'static, {
    Self(Arc::new(move || Box::pin(f()) as BoxFuture<'static, MailboxHandle>))
  }

  pub async fn run(&self) -> MailboxHandle {
    (self.0)().await
  }
}

#[derive(Debug)]
struct DefaultMailboxInner {
  user_mailbox_sender: Arc<Mutex<dyn QueueWriter<MessageHandle>>>,
  user_mailbox_receiver: Arc<Mutex<dyn QueueReader<MessageHandle>>>,
  system_mailbox_sender: Arc<Mutex<dyn QueueWriter<MessageHandle>>>,
  system_mailbox_receiver: Arc<Mutex<dyn QueueReader<MessageHandle>>>,
  scheduler_status: Arc<AtomicBool>,
  user_messages_count: Arc<AtomicI32>,
  system_messages_count: Arc<AtomicI32>,
  suspended: Arc<AtomicBool>,
  invoker_opt: Arc<Mutex<Option<MessageInvokerHandle>>>,
  dispatcher_opt: Arc<Mutex<Option<DispatcherHandle>>>,
  middlewares: Vec<MailboxMiddlewareHandle>,
}

// DefaultMailbox implementation
#[derive(Debug, Clone)]
pub(crate) struct DefaultMailbox {
  inner: Arc<Mutex<DefaultMailboxInner>>,
}

impl DefaultMailbox {
  pub(crate) fn new(
    user_mailbox: impl QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Clone + 'static,
    system_mailbox: impl QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Clone + 'static,
  ) -> Self {
    DefaultMailbox {
      inner: Arc::new(Mutex::new(DefaultMailboxInner {
        user_mailbox_sender: Arc::new(Mutex::new(user_mailbox.clone())),
        user_mailbox_receiver: Arc::new(Mutex::new(user_mailbox)),
        system_mailbox_sender: Arc::new(Mutex::new(system_mailbox.clone())),
        system_mailbox_receiver: Arc::new(Mutex::new(system_mailbox)),
        scheduler_status: Arc::new(AtomicBool::new(false)),
        user_messages_count: Arc::new(AtomicI32::new(0)),
        system_messages_count: Arc::new(AtomicI32::new(0)),
        suspended: Arc::new(AtomicBool::new(false)),
        invoker_opt: Arc::new(Mutex::new(None)),
        dispatcher_opt: Arc::new(Mutex::new(None)),
        middlewares: vec![],
      })),
    }
  }

  pub(crate) async fn with_middlewares(self, middlewares: Vec<MailboxMiddlewareHandle>) -> Self {
    {
      let mut inner_mg = self.inner.lock().await;
      inner_mg.middlewares = middlewares;
    }
    self
  }

  async fn get_message_invoker_opt(&self) -> Option<MessageInvokerHandle> {
    let inner_mg = self.inner.lock().await;
    let invoker_opt_mg = inner_mg.invoker_opt.lock().await;
    invoker_opt_mg.clone()
  }

  async fn set_message_invoker_opt(&mut self, message_invoker: Option<MessageInvokerHandle>) {
    let inner_mg = self.inner.lock().await;
    let mut invoker_opt_mg = inner_mg.invoker_opt.lock().await;
    *invoker_opt_mg = message_invoker;
  }

  async fn get_dispatcher_opt(&self) -> Option<DispatcherHandle> {
    let inner_mg = self.inner.lock().await;
    let dispatcher_opt = inner_mg.dispatcher_opt.lock().await;
    dispatcher_opt.clone()
  }

  async fn set_dispatcher_opt(&mut self, dispatcher_opt: Option<DispatcherHandle>) {
    let inner_mg = self.inner.lock().await;
    let mut dispatcher_opt_mg = inner_mg.dispatcher_opt.lock().await;
    *dispatcher_opt_mg = dispatcher_opt;
  }

  async fn initialize_scheduler_status(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.scheduler_status.store(false, Ordering::SeqCst);
  }

  async fn compare_exchange_scheduler_status(&self, current: bool, new: bool) -> Result<bool, bool> {
    let inner_mg = self.inner.lock().await;
    inner_mg
      .scheduler_status
      .compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst)
  }

  async fn set_suspended(&self, suspended: bool) {
    let inner_mg = self.inner.lock().await;
    inner_mg.suspended.store(suspended, Ordering::SeqCst);
  }

  async fn is_suspended(&self) -> bool {
    let inner_mg = self.inner.lock().await;
    inner_mg.suspended.load(Ordering::SeqCst)
  }

  async fn increment_system_messages_count(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.system_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  async fn decrement_system_messages_count(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.system_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn get_system_messages_count(&self) -> i32 {
    let inner_mg = self.inner.lock().await;
    inner_mg.system_messages_count.load(Ordering::SeqCst)
  }

  async fn increment_user_messages_count(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.user_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  async fn decrement_user_messages_count(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.user_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn get_user_messages_count(&self) -> i32 {
    let inner_mg = self.inner.lock().await;
    inner_mg.user_messages_count.load(Ordering::SeqCst)
  }

  async fn get_middlewares(&self) -> Vec<MailboxMiddlewareHandle> {
    let inner_mg = self.inner.lock().await;
    inner_mg.middlewares.clone()
  }

  async fn poll_system_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let inner_mg = self.inner.lock().await;
    let mut system_mailbox_receiver_mg = inner_mg.system_mailbox_receiver.lock().await;
    system_mailbox_receiver_mg.poll().await
  }

  async fn poll_user_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let inner_mg = self.inner.lock().await;
    let mut user_mailbox_receiver_mg = inner_mg.user_mailbox_receiver.lock().await;
    user_mailbox_receiver_mg.poll().await
  }

  async fn offer_system_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let inner_mg = self.inner.lock().await;
    let mut system_mailbox_sender_mg = inner_mg.system_mailbox_sender.lock().await;
    system_mailbox_sender_mg.offer(element).await
  }

  async fn offer_user_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let inner_mg = self.inner.lock().await;
    let mut user_mailbox_sender_mg = inner_mg.user_mailbox_sender.lock().await;
    user_mailbox_sender_mg.offer(element).await
  }

  async fn schedule(&self) {
    if self.compare_exchange_scheduler_status(false, true).await.is_ok() {
      let dispatcher = self.get_dispatcher_opt().await.expect("Dispatcher is not set");
      let self_clone = self.to_handle().await;
      dispatcher
        .schedule(Runnable::new(move || {
          let self_clone = self_clone.clone();
          async move {
            self_clone.process_messages().await;
          }
        }))
        .await;
    }
  }

  async fn run(&self) {
    let mut i = 0;

    if self.get_dispatcher_opt().await.is_none() || self.get_message_invoker_opt().await.is_none() {
      return;
    }

    let dispatcher = self.get_dispatcher_opt().await.clone().expect("Dispatcher is not set");
    let mut message_invoker = self
      .get_message_invoker_opt()
      .await
      .clone()
      .expect("Message invoker is not set");

    let t = dispatcher.throughput().await;

    loop {
      if i > t {
        i = 0;
        tokio::task::yield_now().await;
      }

      i += 1;

      if let Ok(Some(msg)) = self.poll_system_mailbox().await {
        self.decrement_system_messages_count().await;
        let mailbox_message = msg.as_any().downcast_ref::<MailboxMessage>();
        match mailbox_message {
          Some(MailboxMessage::SuspendMailbox) => {
            self.set_suspended(true).await;
          }
          Some(MailboxMessage::ResumeMailbox) => {
            self.set_suspended(false).await;
          }
          _ => {
            let result = message_invoker.invoke_system_message(msg.clone()).await;
            if let Err(e) = result {
              println!("Failed to invoke system message: {:?}", e);
              message_invoker
                .escalate_failure(e.reason().cloned().unwrap(), msg.clone())
                .await;
            }
          }
        }
        for middleware in self.get_middlewares().await {
          middleware.message_received(msg.clone()).await;
        }
        continue;
      }

      if self.is_suspended().await {
        break;
      }

      if let Ok(Some(message)) = self.poll_user_mailbox().await {
        self.decrement_user_messages_count().await;
        let result = message_invoker.invoke_user_message(message.clone()).await;
        if let Err(e) = result {
          println!("Failed to invoke system message: {:?}", e);
          message_invoker
            .escalate_failure(e.reason().cloned().unwrap(), message.clone())
            .await;
        }
        for middleware in self.get_middlewares().await {
          middleware.message_received(message.clone()).await;
        }
      } else {
        break;
      }
    }
  }
}

#[async_trait]
impl Mailbox for DefaultMailbox {
  async fn process_messages(&self) {
    loop {
      self.run().await;

      self.initialize_scheduler_status().await;
      let system_messages_count = self.get_system_messages_count().await;
      let user_messages_count = self.get_user_messages_count().await;

      if system_messages_count > 0 || (!self.is_suspended().await && user_messages_count > 0) {
        if self.compare_exchange_scheduler_status(false, true).await.is_ok() {
          continue;
        }
      }

      break;
    }

    for middleware in self.get_middlewares().await {
      middleware.mailbox_empty().await;
    }
  }

  async fn post_user_message(&self, message: MessageHandle) {
    for middleware in self.get_middlewares().await {
      middleware.message_posted(message.clone()).await;
    }

    if let Err(e) = self.offer_user_mailbox(message).await {
      println!("Failed to send message: {:?}", e);
    } else {
      self.increment_user_messages_count().await;
      self.schedule().await;
    }
  }

  async fn post_system_message(&self, message: MessageHandle) {
    for middleware in self.get_middlewares().await {
      middleware.message_posted(message.clone()).await;
    }

    if let Err(e) = self.offer_system_mailbox(message).await {
      println!("Failed to send message: {:?}", e);
    } else {
      self.increment_system_messages_count().await;
      self.schedule().await;
    }
  }

  async fn register_handlers(&mut self, invoker: Option<MessageInvokerHandle>, dispatcher: Option<DispatcherHandle>) {
    self.set_message_invoker_opt(invoker).await;
    self.set_dispatcher_opt(dispatcher).await;
  }

  async fn start(&self) {
    for middleware in self.get_middlewares().await {
      middleware.mailbox_started().await;
    }
  }

  async fn user_message_count(&self) -> i32 {
    self.get_user_messages_count().await
  }

  async fn to_handle(&self) -> MailboxHandle {
    MailboxHandle::new(self.clone())
  }
}
