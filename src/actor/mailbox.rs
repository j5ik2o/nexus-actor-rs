use std::fmt::Debug;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::sync::Mutex;

use crate::actor::dispatcher::{Dispatcher, DispatcherHandle, Runnable};
use crate::actor::mailbox_middleware::{MailboxMiddleware, MailboxMiddlewareHandle};
use crate::actor::message::{Message, MessageHandle};
use crate::actor::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::messages::MailboxMessage;
use crate::util::queue::{QueueReader, QueueWriter};

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
  scheduler_status: AtomicBool,
  user_messages: AtomicI32,
  sys_messages: AtomicI32,
  suspended: AtomicBool,
  invoker_opt: Arc<Mutex<Option<MessageInvokerHandle>>>,
  dispatcher_opt: Arc<Mutex<Option<DispatcherHandle>>>,
  middlewares: Vec<MailboxMiddlewareHandle>,
}

// DefaultMailbox implementation
#[derive(Debug, Clone)]
pub(crate) struct DefaultMailbox {
  inner: Arc<DefaultMailboxInner>,
}

impl DefaultMailbox {
  pub(crate) fn new(
    user_mailbox: impl QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Clone + 'static,
    system_mailbox: impl QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Clone + 'static,
  ) -> Self {
    DefaultMailbox {
      inner: Arc::new(DefaultMailboxInner {
        user_mailbox_sender: Arc::new(Mutex::new(user_mailbox.clone())),
        user_mailbox_receiver: Arc::new(Mutex::new(user_mailbox)),
        system_mailbox_sender: Arc::new(Mutex::new(system_mailbox.clone())),
        system_mailbox_receiver: Arc::new(Mutex::new(system_mailbox)),
        scheduler_status: AtomicBool::new(false),
        user_messages: AtomicI32::new(0),
        sys_messages: AtomicI32::new(0),
        suspended: AtomicBool::new(false),
        invoker_opt: Arc::new(Mutex::new(None)),
        dispatcher_opt: Arc::new(Mutex::new(None)),
        middlewares: Vec::new(),
      }),
    }
  }

  pub(crate) fn with_middlewares(mut self, middlewares: Vec<MailboxMiddlewareHandle>) -> Self {
    DefaultMailbox {
      inner: Arc::new(DefaultMailboxInner {
        user_mailbox_sender: self.inner.user_mailbox_sender.clone(),
        user_mailbox_receiver: self.inner.user_mailbox_receiver.clone(),
        system_mailbox_sender: self.inner.system_mailbox_sender.clone(),
        system_mailbox_receiver: self.inner.system_mailbox_receiver.clone(),
        scheduler_status: AtomicBool::new(false),
        user_messages: AtomicI32::new(0),
        sys_messages: AtomicI32::new(0),
        suspended: AtomicBool::new(false),
        invoker_opt: Arc::new(Mutex::new(None)),
        dispatcher_opt: Arc::new(Mutex::new(None)),
        middlewares,
      }),
    }
  }

  async fn get_message_invoker_opt(&self) -> Option<MessageInvokerHandle> {
    self.inner.invoker_opt.lock().await.clone()
  }

  async fn set_message_invoker_opt(&mut self, message_invoker: Option<MessageInvokerHandle>) {
    *self.inner.invoker_opt.lock().await = message_invoker;
  }

  async fn get_dispatcher_opt(&self) -> Option<DispatcherHandle> {
    self.inner.dispatcher_opt.lock().await.clone()
  }

  async fn set_dispatcher_opt(&mut self, dispatcher_opt: Option<DispatcherHandle>) {
    *self.inner.dispatcher_opt.lock().await = dispatcher_opt;
  }

  fn compare_exchange_scheduler_status(&self, current: bool,
                            new: bool,
                            success: Ordering,
                            failure: Ordering) -> Result<bool, bool> {
    self
        .inner
        .scheduler_status
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
  }

  async fn schedule(&self) {
    if self
      .compare_exchange_scheduler_status(false, true, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      if let Some(dispatcher) = self.get_dispatcher_opt().await {
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
  }

  async fn run(&self) {
    let mut i = 0;

    if self.get_dispatcher_opt().await.is_none() || self.get_message_invoker_opt().await.is_none() {
      return;
    }

    let dispatcher = self.get_dispatcher_opt().await.unwrap().clone();
    let mut message_invoker = self.get_message_invoker_opt().await.unwrap().clone();

    let t = dispatcher.throughput().await;

    loop {
      if i > t {
        i = 0;
        tokio::task::yield_now().await;
      }

      i += 1;

      if let Ok(Some(msg)) = {
        let mut smr = self.inner.system_mailbox_receiver.lock().await;
        smr.poll().await
      } {
        self.inner.sys_messages.fetch_sub(1, Ordering::SeqCst);
        let mailbox_message = msg.as_any().downcast_ref::<MailboxMessage>();
        match mailbox_message {
          Some(MailboxMessage::SuspendMailbox) => {
            self.inner.suspended.store(true, Ordering::SeqCst);
          }
          Some(MailboxMessage::ResumeMailbox) => {
            self.inner.suspended.store(false, Ordering::SeqCst);
          }
          _ => {
            message_invoker.invoke_system_message(msg.clone()).await;
          }
        }
        for middleware in &self.inner.middlewares {
          middleware.message_received(msg.clone()).await;
        }
        continue;
      }

      if self.inner.suspended.load(Ordering::SeqCst) {
        break;
      }

      if let Ok(Some(msg)) = {
        let mut umr = self.inner.user_mailbox_receiver.lock().await;
        umr.poll().await
      } {
        self.inner.user_messages.fetch_sub(1, Ordering::SeqCst);
        message_invoker.invoke_user_message(msg.clone()).await;
        for middleware in &self.inner.middlewares {
          middleware.message_received(msg.clone()).await;
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

      self.inner.scheduler_status.store(false, Ordering::SeqCst);
      let sys = self.inner.sys_messages.load(Ordering::SeqCst);
      let user = self.inner.user_messages.load(Ordering::SeqCst);

      if sys > 0 || (!self.inner.suspended.load(Ordering::SeqCst) && user > 0) {
        if self
          .compare_exchange_scheduler_status(false, true, Ordering::SeqCst, Ordering::SeqCst)
          .is_ok()
        {
          continue;
        }
      }

      break;
    }

    for middleware in &self.inner.middlewares {
      middleware.mailbox_empty().await;
    }
  }

  async fn post_user_message(&self, message: MessageHandle) {
    for middleware in &self.inner.middlewares {
      middleware.message_posted(message.clone()).await;
    }

    if let Err(e) = {
      let mut ums = self.inner.user_mailbox_sender.lock().await;
      ums.offer(message).await
    } {
      println!("Failed to send message: {:?}", e);
    } else {
      self.inner.user_messages.fetch_add(1, Ordering::SeqCst);
      self.schedule().await;
    }
  }

  async fn post_system_message(&self, message: MessageHandle) {
    for middleware in &self.inner.middlewares {
      middleware.message_posted(message.clone()).await;
    }

    if let Err(e) = {
      let mut sms = self.inner.system_mailbox_sender.lock().await;
      sms.offer(message).await
    } {
      println!("Failed to send message: {:?}", e);
    } else {
      self.inner.sys_messages.fetch_add(1, Ordering::SeqCst);
      self.schedule().await;
    }
  }

  async fn register_handlers(&mut self, invoker: Option<MessageInvokerHandle>, dispatcher: Option<DispatcherHandle>) {
    self.set_message_invoker_opt(invoker).await;
    self.set_dispatcher_opt(dispatcher).await;
  }

  async fn start(&self) {
    for middleware in &self.inner.middlewares {
      middleware.mailbox_started().await;
    }
  }

  async fn user_message_count(&self) -> i32 {
    self.inner.user_messages.load(Ordering::SeqCst)
  }

  async fn to_handle(&self) -> MailboxHandle {
    MailboxHandle::new(self.clone())
  }
}
