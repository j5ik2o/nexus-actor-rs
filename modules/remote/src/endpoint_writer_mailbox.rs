use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use nexus_actor_std_rs::actor::context::SenderPart;
use nexus_actor_std_rs::actor::core::ExtendedPid;
use nexus_actor_std_rs::actor::core_types::Message;
use nexus_actor_std_rs::actor::dispatch::{
  DeadLetterEvent, Dispatcher, DispatcherHandle, Mailbox, MailboxHandle, MailboxMessage, MailboxQueueLatencyMetrics,
  MailboxSuspensionMetrics, MailboxSync, MailboxSyncHandle, MessageInvoker, MessageInvokerHandle, Runnable,
};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::generated::actor::DeadLetterResponse;
use nexus_utils_std_rs::collections::{
  MpscUnboundedChannelQueue, QueueBase, QueueError, QueueReader, QueueWriter, RingQueue,
};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use crate::messages::{EndpointEvent, RemoteDeliver};
use crate::remote::Remote;

#[derive(Debug, Clone)]
pub struct EndpointWriterMailbox {
  user_mailbox: Arc<Mutex<RingQueue<MessageHandle>>>,
  system_mailbox: Arc<Mutex<MpscUnboundedChannelQueue<MessageHandle>>>,
  scheduler_status: Arc<AtomicBool>,
  has_more_messages: Arc<AtomicI32>,
  batch_size: Arc<AtomicUsize>,
  suspended: Arc<AtomicBool>,
  invoker_opt: Arc<ArcSwapOption<MessageInvokerHandle>>,
  dispatcher_opt: Arc<ArcSwapOption<DispatcherHandle>>,
  queue_capacity: usize,
  remote: Weak<Remote>,
  user_messages_count: Arc<AtomicI32>,
  system_messages_count: Arc<AtomicI32>,
  queue_state_snapshot_interval: Arc<AtomicUsize>,
  queue_state_snapshot_counter: Arc<AtomicU64>,
  queue_state_last_len: Arc<AtomicUsize>,
}

impl EndpointWriterMailbox {
  pub fn new(remote: Weak<Remote>, batch_size: usize, queue_capacity: usize, queue_snapshot_interval: usize) -> Self {
    assert!(queue_capacity > 0, "queue_capacity must be greater than zero");
    let ring_queue = RingQueue::new(queue_capacity).with_dynamic(false);
    let user_mailbox = Arc::new(Mutex::new(ring_queue));
    let system_mailbox = Arc::new(Mutex::new(MpscUnboundedChannelQueue::new()));
    Self {
      user_mailbox,
      system_mailbox,
      scheduler_status: Arc::new(AtomicBool::new(false)),
      has_more_messages: Arc::new(AtomicI32::new(0)),
      batch_size: Arc::new(AtomicUsize::new(batch_size)),
      suspended: Arc::new(AtomicBool::new(false)),
      invoker_opt: Arc::new(ArcSwapOption::from(None)),
      dispatcher_opt: Arc::new(ArcSwapOption::from(None)),
      queue_capacity,
      remote,
      user_messages_count: Arc::new(AtomicI32::new(0)),
      system_messages_count: Arc::new(AtomicI32::new(0)),
      queue_state_snapshot_interval: Arc::new(AtomicUsize::new(queue_snapshot_interval.max(1))),
      queue_state_snapshot_counter: Arc::new(AtomicU64::new(0)),
      queue_state_last_len: Arc::new(AtomicUsize::new(0)),
    }
  }

  fn dispatcher_handle(&self) -> Option<DispatcherHandle> {
    self.dispatcher_opt.load_full().map(|handle| handle.as_ref().clone())
  }

  fn set_dispatcher_handle(&self, dispatcher: Option<DispatcherHandle>) {
    self.dispatcher_opt.store(dispatcher.map(|handle| Arc::new(handle)));
  }

  fn message_invoker_handle(&self) -> Option<MessageInvokerHandle> {
    self.invoker_opt.load_full().map(|handle| handle.as_ref().clone())
  }

  fn set_message_invoker_handle(&self, handle: Option<MessageInvokerHandle>) {
    self.invoker_opt.store(handle.map(|h| Arc::new(h)));
  }

  fn is_suspended(&self) -> bool {
    self.suspended.load(Ordering::SeqCst)
  }

  fn set_suspended(&self, value: bool) {
    self.suspended.store(value, Ordering::SeqCst);
  }

  async fn poll_system_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let result = {
      let mut mailbox = self.system_mailbox.lock();
      QueueReader::poll(&mut *mailbox)
    };
    if let Ok(Some(_)) = &result {
      self.system_messages_count.fetch_sub(1, Ordering::SeqCst);
    }
    result
  }

  async fn poll_user_mailbox(&self) -> Result<Vec<MessageHandle>, QueueError<MessageHandle>> {
    let batch_size = self.batch_size.load(Ordering::SeqCst).max(1);
    let result = {
      let mut mailbox = self.user_mailbox.lock();
      let mut messages = Vec::with_capacity(batch_size);
      for _ in 0..batch_size {
        match QueueReader::poll(&mut *mailbox)? {
          Some(message) => messages.push(message),
          None => break,
        }
      }
      Ok::<_, QueueError<MessageHandle>>(messages)
    }?;

    let len = result.len();
    if len > 0 {
      self.user_messages_count.fetch_sub(len as i32, Ordering::SeqCst);
    }
    Ok(result)
  }

  async fn schedule(&self) {
    self.has_more_messages.store(1, std::sync::atomic::Ordering::SeqCst);
    if self
      .scheduler_status
      .compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
      )
      .is_ok()
    {
      let dispatcher = self.dispatcher_handle().expect("Dispatcher is not set");
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
    if self.message_invoker_handle().is_none() {
      return;
    }

    let mut message_invoker = self.message_invoker_handle().expect("Message invoker is not set");

    loop {
      if let Ok(Some(msg)) = self.poll_system_mailbox().await {
        let mailbox_message = msg.to_typed::<MailboxMessage>();
        match mailbox_message {
          Some(MailboxMessage::SuspendMailbox) => {
            self.set_suspended(true);
          }
          Some(MailboxMessage::ResumeMailbox) => {
            self.set_suspended(false);
          }
          _ => {
            if let Err(err) = message_invoker.invoke_system_message(msg.clone()).await {
              message_invoker
                .escalate_failure(err.reason().cloned().unwrap(), msg.clone())
                .await;
            }
          }
        }
        continue;
      }

      if self.is_suspended() {
        break;
      }

      let user_messages = match self.poll_user_mailbox().await {
        Ok(messages) => messages,
        Err(err) => {
          tracing::error!("EndpointWriterMailbox failed to poll user mailbox: {:?}", err);
          Vec::new()
        }
      };

      if user_messages.is_empty() {
        break;
      }

      let mut address_hint: Option<String> = None;
      for msg in &user_messages {
        if address_hint.is_none() {
          address_hint = Self::extract_endpoint_address(msg);
        }

        if let Err(err) = message_invoker.invoke_user_message(msg.clone()).await {
          message_invoker
            .escalate_failure(err.reason().cloned().unwrap(), msg.clone())
            .await;
        }
      }

      if let Some(address) = address_hint {
        self.record_queue_state_for_address(&address).await;
      }

      tokio::task::yield_now().await;
    }
  }

  async fn handle_overflow(&self, message_handle: MessageHandle) {
    self.increment_dead_letter_for_message(&message_handle).await;
    self.record_queue_state_for_message(&message_handle, true).await;
    tracing::warn!(
      "EndpointWriterMailbox queue full; dropping message: {:?}",
      message_handle
    );
    let Some(remote) = self.remote.upgrade() else {
      tracing::warn!("Remote has been dropped; unable to deliver DeadLetter for overflow");
      return;
    };

    let actor_system = remote.get_actor_system().clone();

    if let Some(remote_deliver) = message_handle.to_typed::<RemoteDeliver>() {
      let mut root = actor_system.get_root_context().await;
      if let Some(sender) = remote_deliver.sender {
        let sender = ExtendedPid::new(sender);
        root
          .send(
            sender,
            MessageHandle::new(DeadLetterResponse {
              target: Some(remote_deliver.target.clone()),
            }),
          )
          .await;
      } else {
        actor_system
          .get_event_stream()
          .await
          .publish(MessageHandle::new(DeadLetterEvent {
            pid: Some(ExtendedPid::new(remote_deliver.target)),
            message_handle: remote_deliver.message.clone(),
            sender: None,
          }))
          .await;
      }
      return;
    }

    if let Some(endpoint_event) = message_handle.to_typed::<EndpointEvent>() {
      actor_system
        .get_event_stream()
        .await
        .publish(MessageHandle::new(endpoint_event))
        .await;
      return;
    }

    actor_system.get_event_stream().await.publish(message_handle).await;
  }

  fn extract_endpoint_address(message_handle: &MessageHandle) -> Option<String> {
    if let Some(remote_deliver) = message_handle.to_typed::<RemoteDeliver>() {
      return Some(remote_deliver.target.address);
    }

    if let Some(endpoint_event) = message_handle.to_typed::<EndpointEvent>() {
      return match endpoint_event {
        EndpointEvent::EndpointConnected(ev) => Some(ev.address),
        EndpointEvent::EndpointTerminated(ev) => Some(ev.address),
      };
    }

    None
  }

  async fn record_queue_state_for_message(&self, message_handle: &MessageHandle, force: bool) {
    if let Some(address) = Self::extract_endpoint_address(message_handle) {
      self.record_queue_state_for_address_internal(&address, force).await;
    }
  }

  async fn record_queue_state_for_address(&self, address: &str) {
    self.record_queue_state_for_address_internal(address, false).await;
  }

  async fn record_queue_state_for_address_internal(&self, address: &str, force: bool) {
    let queue_len = {
      let mailbox = self.user_mailbox.lock();
      let len = QueueBase::len(&*mailbox).to_usize();
      drop(mailbox);
      len
    };

    if !self.should_record_queue_state(queue_len, force) {
      return;
    }

    if let Some(remote) = self.remote.upgrade() {
      if let Some(manager) = remote.get_endpoint_manager_opt().await {
        manager
          .record_queue_state(address, self.queue_capacity, queue_len)
          .await;
      }
    }
  }

  async fn increment_dead_letter_for_message(&self, message_handle: &MessageHandle) {
    if let Some(address) = Self::extract_endpoint_address(message_handle) {
      if let Some(remote) = self.remote.upgrade() {
        if let Some(manager) = remote.get_endpoint_manager_opt().await {
          manager.increment_dead_letter(&address).await;
        }
      }
    }
  }

  fn should_record_queue_state(&self, len: usize, force: bool) -> bool {
    if force {
      self.queue_state_snapshot_counter.store(0, Ordering::Relaxed);
      self.queue_state_last_len.store(len, Ordering::Relaxed);
      return true;
    }

    let interval = self.queue_state_snapshot_interval.load(Ordering::Relaxed).max(1);
    let previous = self.queue_state_last_len.load(Ordering::Relaxed);

    if interval == 1 {
      if previous == len {
        return false;
      }
      self.queue_state_last_len.store(len, Ordering::Relaxed);
      return true;
    }

    if len == previous {
      return false;
    }

    if len == 0 || len >= self.queue_capacity {
      self.queue_state_snapshot_counter.store(0, Ordering::Relaxed);
      self.queue_state_last_len.store(len, Ordering::Relaxed);
      return true;
    }

    if previous == 0 && len > 0 {
      self.queue_state_snapshot_counter.store(1, Ordering::Relaxed);
      self.queue_state_last_len.store(len, Ordering::Relaxed);
      return true;
    }

    let warning_threshold = (self.queue_capacity.saturating_mul(3) + 3) / 4;
    let warning_threshold = warning_threshold.max(1);
    if len >= warning_threshold && len > previous {
      self.queue_state_snapshot_counter.store(0, Ordering::Relaxed);
      self.queue_state_last_len.store(len, Ordering::Relaxed);
      return true;
    }

    let counter = self.queue_state_snapshot_counter.fetch_add(1, Ordering::Relaxed) + 1;
    if counter >= interval as u64 {
      self.queue_state_snapshot_counter.store(0, Ordering::Relaxed);
      self.queue_state_last_len.store(len, Ordering::Relaxed);
      return true;
    }

    false
  }
}

impl MailboxSync for EndpointWriterMailbox {
  fn user_messages_count(&self) -> i32 {
    self.user_messages_count.load(Ordering::SeqCst)
  }

  fn system_messages_count(&self) -> i32 {
    self.system_messages_count.load(Ordering::SeqCst)
  }

  fn suspension_metrics(&self) -> MailboxSuspensionMetrics {
    MailboxSuspensionMetrics::default()
  }

  fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics {
    MailboxQueueLatencyMetrics::default()
  }

  fn is_suspended(&self) -> bool {
    self.is_suspended()
  }
}

#[async_trait]
impl Mailbox for EndpointWriterMailbox {
  async fn get_user_messages_count(&self) -> i32 {
    self.user_messages_count.load(Ordering::SeqCst)
  }

  async fn get_system_messages_count(&self) -> i32 {
    self.system_messages_count.load(Ordering::SeqCst)
  }

  async fn process_messages(&self) {
    self.has_more_messages.store(0, Ordering::SeqCst);
    loop {
      // Mirror protoactor-go/remote/endpoint_writer_mailbox.go::processMessages control flow.
      self.run().await;
      self.scheduler_status.store(false, Ordering::SeqCst);
      let has_more = self.has_more_messages.swap(0, Ordering::SeqCst) == 1;
      if has_more
        && self
          .scheduler_status
          .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
          .is_ok()
      {
        continue;
      }
      break;
    }
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    tracing::trace!(message_type = %message_handle.get_type_name(), "EndpointWriterMailbox::post_user_message");
    let address_hint = Self::extract_endpoint_address(&message_handle);
    let enqueue_result = {
      let mut mailbox = self.user_mailbox.lock();
      let len = QueueBase::len(&*mailbox).to_usize();
      if len >= self.queue_capacity {
        Err(QueueError::OfferError(message_handle.clone()))
      } else {
        QueueWriter::offer(&mut *mailbox, message_handle.clone())
      }
    };

    if let Err(err) = enqueue_result {
      match err {
        QueueError::OfferError(message) => {
          self.handle_overflow(message).await;
        }
        other => {
          tracing::error!(error = ?other, "EndpointWriterMailbox failed to enqueue user message");
          self.handle_overflow(message_handle).await;
        }
      }
      return;
    }

    self.user_messages_count.fetch_add(1, Ordering::SeqCst);
    if let Some(address) = address_hint {
      self.record_queue_state_for_address(&address).await;
    }
    self.schedule().await;
  }

  async fn post_system_message(&self, message_handle: MessageHandle) {
    tracing::trace!(message = %message_handle.get_type_name(), "EndpointWriterMailbox::post_system_message");
    if let Err(err) = {
      let mut mailbox = self.system_mailbox.lock();
      QueueWriter::offer(&mut *mailbox, message_handle)
    } {
      tracing::error!(error = ?err, "EndpointWriterMailbox failed to enqueue system message");
      return;
    }
    self.system_messages_count.fetch_add(1, Ordering::SeqCst);
    self.schedule().await;
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    self.set_message_invoker_handle(message_invoker_handle);
    self.set_dispatcher_handle(dispatcher_handle);
    self.queue_state_snapshot_counter.store(0, Ordering::Relaxed);
    self.queue_state_last_len.store(0, Ordering::Relaxed);
  }

  async fn start(&self) {}

  async fn user_message_count(&self) -> i32 {
    self.user_messages_count.load(Ordering::SeqCst)
  }

  async fn to_handle(&self) -> MailboxHandle {
    let mailbox_clone = self.clone();
    let sync = MailboxSyncHandle::new(mailbox_clone.clone());
    MailboxHandle::new_with_sync(mailbox_clone, Some(sync))
  }
}
