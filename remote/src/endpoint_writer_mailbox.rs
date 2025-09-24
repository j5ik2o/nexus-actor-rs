use async_trait::async_trait;
use nexus_actor_core_rs::actor::context::SenderPart;
use nexus_actor_core_rs::actor::core::ExtendedPid;
use nexus_actor_core_rs::actor::dispatch::{
  DeadLetterEvent, Dispatcher, DispatcherHandle, Mailbox, MailboxHandle, MailboxMessage, MessageInvoker,
  MessageInvokerHandle, Runnable,
};
use nexus_actor_core_rs::actor::message::MessageHandle;
use nexus_actor_core_rs::generated::actor::DeadLetterResponse;
use nexus_actor_utils_rs::collections::{
  MpscUnboundedChannelQueue, QueueBase, QueueError, QueueReader, QueueWriter, RingQueue,
};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

use crate::messages::{EndpointEvent, RemoteDeliver};
use crate::remote::Remote;

#[derive(Debug, Clone)]
pub struct EndpointWriterMailbox {
  user_mailbox: Arc<RwLock<RingQueue<MessageHandle>>>,
  system_mailbox: Arc<RwLock<MpscUnboundedChannelQueue<MessageHandle>>>,
  scheduler_status: Arc<AtomicBool>,
  has_more_messages: Arc<AtomicI32>,
  batch_size: Arc<AtomicUsize>,
  suspended: Arc<AtomicBool>,
  invoker_opt: Arc<RwLock<Option<MessageInvokerHandle>>>,
  dispatcher_opt: Arc<RwLock<Option<DispatcherHandle>>>,
  queue_capacity: usize,
  remote: Weak<Remote>,
}

impl EndpointWriterMailbox {
  pub fn new(remote: Weak<Remote>, batch_size: usize, queue_capacity: usize) -> Self {
    assert!(queue_capacity > 0, "queue_capacity must be greater than zero");
    let ring_queue = RingQueue::new(queue_capacity).with_dynamic(false);
    let user_mailbox = Arc::new(RwLock::new(ring_queue));
    let system_mailbox = Arc::new(RwLock::new(MpscUnboundedChannelQueue::new()));
    Self {
      user_mailbox,
      system_mailbox,
      scheduler_status: Arc::new(AtomicBool::new(false)),
      has_more_messages: Arc::new(AtomicI32::new(0)),
      batch_size: Arc::new(AtomicUsize::new(batch_size)),
      suspended: Arc::new(AtomicBool::new(false)),
      invoker_opt: Arc::new(RwLock::new(None)),
      dispatcher_opt: Arc::new(RwLock::new(None)),
      queue_capacity,
      remote,
    }
  }

  async fn get_dispatcher_opt(&self) -> Option<DispatcherHandle> {
    let inner_mg = self.dispatcher_opt.read().await;
    inner_mg.clone()
  }

  async fn get_message_invoker_opt(&self) -> Option<MessageInvokerHandle> {
    let mg = self.invoker_opt.read().await;
    mg.clone()
  }

  fn is_suspended(&self) -> bool {
    self.suspended.load(Ordering::SeqCst)
  }

  fn set_suspended(&self, value: bool) {
    self.suspended.store(value, Ordering::SeqCst);
  }

  async fn poll_system_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let mut mg = self.system_mailbox.write().await;
    mg.poll().await
  }

  async fn poll_user_mailbox(&self) -> Result<Vec<MessageHandle>, QueueError<MessageHandle>> {
    let mut mg = self.user_mailbox.write().await;
    let batch_size = self.batch_size.load(Ordering::SeqCst).max(1);
    mg.poll_many(batch_size).await
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
    if self.get_message_invoker_opt().await.is_none() {
      return;
    }

    let mut message_invoker = self
      .get_message_invoker_opt()
      .await
      .clone()
      .expect("Message invoker is not set");

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
    self.record_queue_state_for_message(&message_handle).await;
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

  async fn record_queue_state_for_message(&self, message_handle: &MessageHandle) {
    if let Some(address) = Self::extract_endpoint_address(message_handle) {
      self.record_queue_state_for_address(&address).await;
    }
  }

  async fn record_queue_state_for_address(&self, address: &str) {
    let queue_len = {
      let mailbox = self.user_mailbox.read().await;
      let len = mailbox.len().await.to_usize();
      drop(mailbox);
      len
    };

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
          manager.increment_dead_letter(&address);
        }
      }
    }
  }
}

#[async_trait]
impl Mailbox for EndpointWriterMailbox {
  async fn get_user_messages_count(&self) -> i32 {
    let mg = self.user_mailbox.read().await;
    mg.len().await.to_usize() as i32
  }

  async fn get_system_messages_count(&self) -> i32 {
    let mg = self.system_mailbox.read().await;
    mg.len().await.to_usize() as i32
  }

  async fn process_messages(&self) {
    self.has_more_messages.store(0, Ordering::SeqCst);
    loop {
      // Mirror protoactor-go/remote/endpoint_writer_mailbox.go::processMessages control flow.
      self.run().await;
      self.scheduler_status.store(false, Ordering::SeqCst);
      let has_more = self.has_more_messages.swap(0, Ordering::SeqCst) == 1;
      if has_more {
        if self
          .scheduler_status
          .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
          .is_ok()
        {
          continue;
        }
      }
      break;
    }
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    tracing::info!("EndpointWriterMailbox::post_user_message: {:?}", message_handle);
    let address_hint = Self::extract_endpoint_address(&message_handle);
    {
      let mut mg = self.user_mailbox.write().await;
      let len = mg.len().await.to_usize();
      if len >= self.queue_capacity {
        drop(mg);
        self.handle_overflow(message_handle).await;
        return;
      }

      if let Err(QueueError::OfferError(message)) = mg.offer(message_handle).await {
        drop(mg);
        self.handle_overflow(message).await;
        return;
      }
    }
    if let Some(address) = address_hint {
      self.record_queue_state_for_address(&address).await;
    }
    self.schedule().await;
  }

  async fn post_system_message(&self, message_handle: MessageHandle) {
    tracing::info!("EndpointWriterMailbox::post_system_message: {:?}", message_handle);
    {
      let mut mg = self.system_mailbox.write().await;
      mg.offer(message_handle).await.unwrap();
    }
    self.schedule().await;
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    {
      let mut invoker_opt = self.invoker_opt.write().await;
      *invoker_opt = message_invoker_handle;
    }
    {
      let mut dispatcher_opt = self.dispatcher_opt.write().await;
      *dispatcher_opt = dispatcher_handle;
    }
  }

  async fn start(&self) {}

  async fn user_message_count(&self) -> i32 {
    self.get_user_messages_count().await
  }

  async fn to_handle(&self) -> MailboxHandle {
    MailboxHandle::new(self.clone())
  }
}
