use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

use crate::actor::dispatch::dispatcher::{Dispatcher, DispatcherHandle, Runnable};
use crate::actor::dispatch::mailbox::mailbox_handle::MailboxHandle;
use crate::actor::dispatch::mailbox::Mailbox;
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::mailbox_middleware::{MailboxMiddleware, MailboxMiddlewareHandle};
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use nexus_actor_utils_rs::collections::{QueueError, QueueReader, QueueWriter};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone)]
struct QueueWriterHandle {
  inner: Arc<Mutex<dyn QueueWriter<MessageHandle> + Send + Sync>>,
}

impl QueueWriterHandle {
  fn new(inner: Arc<Mutex<dyn QueueWriter<MessageHandle> + Send + Sync>>) -> Self {
    Self { inner }
  }

  async fn offer(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let mut guard = self.inner.lock().await;
    guard.offer(element).await
  }
}

#[derive(Debug, Clone)]
struct QueueReaderHandle {
  inner: Arc<Mutex<dyn QueueReader<MessageHandle> + Send + Sync>>,
}

impl QueueReaderHandle {
  fn new(inner: Arc<Mutex<dyn QueueReader<MessageHandle> + Send + Sync>>) -> Self {
    Self { inner }
  }

  async fn poll(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let mut guard = self.inner.lock().await;
    guard.poll().await
  }
}

#[derive(Debug)]
pub(crate) struct DefaultMailboxInner {
  user_mailbox_sender: QueueWriterHandle,
  user_mailbox_receiver: QueueReaderHandle,
  system_mailbox_sender: QueueWriterHandle,
  system_mailbox_receiver: QueueReaderHandle,
  scheduler_status: Arc<AtomicBool>,
  user_messages_count: Arc<AtomicI32>,
  system_messages_count: Arc<AtomicI32>,
  suspended: Arc<AtomicBool>,
  invoker_opt: Arc<RwLock<Option<MessageInvokerHandle>>>,
  dispatcher_opt: Arc<RwLock<Option<DispatcherHandle>>>,
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
    Self {
      inner: Arc::new(Mutex::new(DefaultMailboxInner {
        user_mailbox_sender: QueueWriterHandle::new(Arc::new(Mutex::new(user_mailbox.clone()))),
        user_mailbox_receiver: QueueReaderHandle::new(Arc::new(Mutex::new(user_mailbox))),
        system_mailbox_sender: QueueWriterHandle::new(Arc::new(Mutex::new(system_mailbox.clone()))),
        system_mailbox_receiver: QueueReaderHandle::new(Arc::new(Mutex::new(system_mailbox))),
        scheduler_status: Arc::new(AtomicBool::new(false)),
        user_messages_count: Arc::new(AtomicI32::new(0)),
        system_messages_count: Arc::new(AtomicI32::new(0)),
        suspended: Arc::new(AtomicBool::new(false)),
        invoker_opt: Arc::new(RwLock::new(None)),
        dispatcher_opt: Arc::new(RwLock::new(None)),
        middlewares: vec![],
      })),
    }
  }

  pub(crate) async fn with_middlewares(self, middlewares: impl IntoIterator<Item = MailboxMiddlewareHandle>) -> Self {
    {
      let mut inner_mg = self.inner.lock().await;
      inner_mg.middlewares = middlewares.into_iter().collect();
    }
    self
  }

  async fn get_message_invoker_opt(&self) -> Option<MessageInvokerHandle> {
    let inner_mg = self.inner.lock().await;
    let invoker_opt_mg = inner_mg.invoker_opt.read().await;
    invoker_opt_mg.clone()
  }

  async fn set_message_invoker_opt(&mut self, message_invoker: Option<MessageInvokerHandle>) {
    let inner_mg = self.inner.lock().await;
    let mut invoker_opt_mg = inner_mg.invoker_opt.write().await;
    *invoker_opt_mg = message_invoker;
  }

  async fn get_dispatcher_opt(&self) -> Option<DispatcherHandle> {
    let inner_mg = self.inner.lock().await;
    let dispatcher_opt = inner_mg.dispatcher_opt.read().await;
    dispatcher_opt.clone()
  }

  async fn set_dispatcher_opt(&mut self, dispatcher_opt: Option<DispatcherHandle>) {
    let inner_mg = self.inner.lock().await;
    let mut dispatcher_opt_mg = inner_mg.dispatcher_opt.write().await;
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

  async fn increment_user_messages_count(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.user_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  async fn decrement_user_messages_count(&self) {
    let inner_mg = self.inner.lock().await;
    inner_mg.user_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn get_middlewares(&self) -> Vec<MailboxMiddlewareHandle> {
    let inner_mg = self.inner.lock().await;
    inner_mg.middlewares.clone()
  }

  async fn poll_system_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let receiver = {
      let inner_mg = self.inner.lock().await;
      inner_mg.system_mailbox_receiver.clone()
    };
    receiver.poll().await
  }

  async fn poll_user_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let receiver = {
      let inner_mg = self.inner.lock().await;
      inner_mg.user_mailbox_receiver.clone()
    };
    receiver.poll().await
  }

  async fn offer_system_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let sender = {
      let inner_mg = self.inner.lock().await;
      inner_mg.system_mailbox_sender.clone()
    };
    sender.offer(element).await
  }

  async fn offer_user_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let sender = {
      let inner_mg = self.inner.lock().await;
      inner_mg.user_mailbox_sender.clone()
    };
    sender.offer(element).await
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
        let mailbox_message = msg.to_typed::<MailboxMessage>();
        match mailbox_message {
          Some(MailboxMessage::SuspendMailbox) => {
            self.set_suspended(true).await;
          }
          Some(MailboxMessage::ResumeMailbox) => {
            self.set_suspended(false).await;
          }
          _ => {
            if let Err(err) = message_invoker.invoke_system_message(msg.clone()).await {
              message_invoker
                .escalate_failure(err.reason().cloned().unwrap(), msg.clone())
                .await;
            }
          }
        }
        let mut middlewares = self.get_middlewares().await;
        for middleware in &mut middlewares {
          middleware.message_received(&msg).await;
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
          message_invoker
            .escalate_failure(e.reason().cloned().unwrap(), message.clone())
            .await;
        }
        let mut middlewares = self.get_middlewares().await;
        for middleware in &mut middlewares {
          middleware.message_received(&message).await;
        }
      } else {
        break;
      }
    }
  }
}

#[async_trait]
impl Mailbox for DefaultMailbox {
  async fn get_user_messages_count(&self) -> i32 {
    let inner_mg = self.inner.lock().await;
    inner_mg.user_messages_count.load(Ordering::SeqCst)
  }

  async fn get_system_messages_count(&self) -> i32 {
    let inner_mg = self.inner.lock().await;
    inner_mg.system_messages_count.load(Ordering::SeqCst)
  }

  async fn process_messages(&self) {
    loop {
      self.run().await;

      self.initialize_scheduler_status().await;
      let system_messages_count = self.get_system_messages_count().await;
      let user_messages_count = self.get_user_messages_count().await;

      if (system_messages_count > 0 || (!self.is_suspended().await && user_messages_count > 0))
        && self.compare_exchange_scheduler_status(false, true).await.is_ok()
      {
        continue;
      }

      // if system_messages_count > 0 || (!self.is_suspended().await && user_messages_count > 0) {
      //   if self.compare_exchange_scheduler_status(false, true).await.is_ok() {
      //     continue;
      //   }
      // }

      break;
    }

    for mut middleware in self.get_middlewares().await {
      middleware.mailbox_empty().await;
    }
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    for mut middleware in self.get_middlewares().await {
      middleware.message_posted(&message_handle).await;
    }

    if let Err(e) = self.offer_user_mailbox(message_handle).await {
      tracing::error!("Failed to send message: {:?}", e);
    } else {
      self.increment_user_messages_count().await;
      self.schedule().await;
    }
  }

  async fn post_system_message(&self, message_handle: MessageHandle) {
    for mut middleware in self.get_middlewares().await {
      middleware.message_posted(&message_handle).await;
    }

    if let Err(e) = self.offer_system_mailbox(message_handle).await {
      tracing::error!("Failed to send message: {:?}", e);
    } else {
      self.increment_system_messages_count().await;
      self.schedule().await;
    }
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    self.set_message_invoker_opt(message_invoker_handle).await;
    self.set_dispatcher_opt(dispatcher_handle).await;
  }

  async fn start(&self) {
    for mut middleware in self.get_middlewares().await {
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
