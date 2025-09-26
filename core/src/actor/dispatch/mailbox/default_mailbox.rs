use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::actor::dispatch::dispatcher::{Dispatcher, DispatcherHandle, Runnable};
use crate::actor::dispatch::mailbox::mailbox_handle::MailboxHandle;
use crate::actor::dispatch::mailbox::Mailbox;
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::mailbox_middleware::{MailboxMiddleware, MailboxMiddlewareHandle};
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::dispatch::MailboxQueueKind;
use crate::actor::message::MessageHandle;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use nexus_actor_utils_rs::collections::{QueueError, QueueReader, QueueWriter};
use tokio::sync::Mutex;

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

#[derive(Debug, Clone)]
struct QueueLatencyTracker {
  timestamps: Arc<Mutex<VecDeque<Instant>>>,
}

impl Default for QueueLatencyTracker {
  fn default() -> Self {
    Self {
      timestamps: Arc::new(Mutex::new(VecDeque::new())),
    }
  }
}

impl QueueLatencyTracker {
  async fn record_enqueue(&self) {
    let mut guard = self.timestamps.lock().await;
    guard.push_back(Instant::now());
  }

  async fn record_dequeue(&self) -> Option<Duration> {
    let mut guard = self.timestamps.lock().await;
    guard.pop_front().map(|instant| instant.elapsed())
  }

  async fn clear(&self) {
    let mut guard = self.timestamps.lock().await;
    guard.clear();
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
  middlewares: Vec<MailboxMiddlewareHandle>,
  user_queue_latency: QueueLatencyTracker,
  system_queue_latency: QueueLatencyTracker,
}

// DefaultMailbox implementation
#[derive(Debug, Clone)]
pub(crate) struct DefaultMailbox {
  inner: Arc<Mutex<DefaultMailboxInner>>,
  invoker_opt: Arc<ArcSwapOption<MessageInvokerHandle>>,
  dispatcher_opt: Arc<ArcSwapOption<DispatcherHandle>>,
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
        middlewares: vec![],
        user_queue_latency: QueueLatencyTracker::default(),
        system_queue_latency: QueueLatencyTracker::default(),
      })),
      invoker_opt: Arc::new(ArcSwapOption::from(None)),
      dispatcher_opt: Arc::new(ArcSwapOption::from(None)),
    }
  }

  pub(crate) async fn with_middlewares(self, middlewares: impl IntoIterator<Item = MailboxMiddlewareHandle>) -> Self {
    {
      let mut inner_mg = self.inner.lock().await;
      inner_mg.middlewares = middlewares.into_iter().collect();
    }
    self
  }

  fn message_invoker_opt(&self) -> Option<MessageInvokerHandle> {
    self
      .invoker_opt
      .load_full()
      .map(|handle_arc| handle_arc.as_ref().clone())
  }

  fn set_message_invoker_opt(&self, message_invoker: Option<MessageInvokerHandle>) {
    self.invoker_opt.store(message_invoker.map(|handle| Arc::new(handle)));
  }

  fn dispatcher_opt(&self) -> Option<DispatcherHandle> {
    self
      .dispatcher_opt
      .load_full()
      .map(|handle_arc| handle_arc.as_ref().clone())
  }

  fn set_dispatcher_opt(&self, dispatcher_opt: Option<DispatcherHandle>) {
    self.dispatcher_opt.store(dispatcher_opt.map(|handle| Arc::new(handle)));
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

  async fn record_queue_enqueue(&self, queue: MailboxQueueKind) {
    let tracker = {
      let inner_mg = self.inner.lock().await;
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.record_enqueue().await;
  }

  async fn record_queue_dequeue(&self, queue: MailboxQueueKind) -> Option<Duration> {
    let tracker = {
      let inner_mg = self.inner.lock().await;
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.record_dequeue().await
  }

  async fn clear_queue_latency(&self, queue: MailboxQueueKind) {
    let tracker = {
      let inner_mg = self.inner.lock().await;
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.clear().await;
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
    match receiver.poll().await {
      Ok(message) => Ok(message),
      Err(err) => {
        if matches!(err, QueueError::PoolError) {
          self.clear_queue_latency(MailboxQueueKind::System).await;
        }
        Err(err)
      }
    }
  }

  async fn poll_user_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let receiver = {
      let inner_mg = self.inner.lock().await;
      inner_mg.user_mailbox_receiver.clone()
    };
    match receiver.poll().await {
      Ok(message) => Ok(message),
      Err(err) => {
        if matches!(err, QueueError::PoolError) {
          self.clear_queue_latency(MailboxQueueKind::User).await;
        }
        Err(err)
      }
    }
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
      let dispatcher = self.dispatcher_opt().expect("Dispatcher is not set");
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

    if self.dispatcher_opt().is_none() || self.message_invoker_opt().is_none() {
      return;
    }

    let dispatcher = self.dispatcher_opt().expect("Dispatcher is not set");
    let mut message_invoker = self.message_invoker_opt().expect("Message invoker is not set");

    let t = dispatcher.throughput().await;

    loop {
      if i > t {
        i = 0;
        tokio::task::yield_now().await;
      }

      i += 1;

      if let Ok(Some(msg)) = self.poll_system_mailbox().await {
        if let Some(latency) = self.record_queue_dequeue(MailboxQueueKind::System).await {
          message_invoker
            .record_mailbox_queue_latency(MailboxQueueKind::System, latency)
            .await;
        }
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
        if let Some(latency) = self.record_queue_dequeue(MailboxQueueKind::User).await {
          message_invoker
            .record_mailbox_queue_latency(MailboxQueueKind::User, latency)
            .await;
        }
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
      self.record_queue_enqueue(MailboxQueueKind::User).await;
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
      self.record_queue_enqueue(MailboxQueueKind::System).await;
      self.schedule().await;
    }
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    self.set_message_invoker_opt(message_invoker_handle);
    self.set_dispatcher_opt(dispatcher_handle);
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
