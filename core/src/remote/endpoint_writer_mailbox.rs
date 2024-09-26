use crate::actor::dispatch::{
  Dispatcher, DispatcherHandle, Mailbox, MailboxHandle, MailboxMessage, MessageInvoker, MessageInvokerHandle, Runnable,
};
use crate::actor::message::MessageHandle;
use crate::util::queue::{MpscUnboundedChannelQueue, QueueBase, QueueError, QueueReader, QueueWriter, RingQueue};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

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
}

impl EndpointWriterMailbox {
  pub fn new(batch_size: usize, initial_size: usize) -> Self {
    let user_mailbox = Arc::new(RwLock::new(RingQueue::new(initial_size)));
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

  async fn poll_user_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let mut mg = self.user_mailbox.write().await;
    mg.poll().await
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

      if let Ok(Some(msg)) = self.poll_user_mailbox().await {
        if let Err(err) = message_invoker.invoke_user_message(msg.clone()).await {
          message_invoker
            .escalate_failure(err.reason().cloned().unwrap(), msg.clone())
            .await;
        }
      } else {
        break;
      }

      tokio::task::yield_now().await;
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
    self.has_more_messages.store(0, std::sync::atomic::Ordering::SeqCst);
    loop {
      self.run().await;
      self.scheduler_status.store(false, std::sync::atomic::Ordering::SeqCst);
      let has_more = self.has_more_messages.swap(0, std::sync::atomic::Ordering::SeqCst) == 1;
      if !has_more
        && self
          .scheduler_status
          .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
          .is_err()
      {
        break;
      }
    }
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    tracing::info!("EndpointWriterMailbox::post_user_message: {:?}", message_handle);
    {
      let mut mg = self.user_mailbox.write().await;
      mg.offer(message_handle).await.unwrap();
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
