use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::core::ActorError;
use crate::actor::core::ErrorReason;
use crate::actor::dispatch::{MailboxQueueKind, MailboxQueueLatencyMetrics, MailboxSuspensionMetrics};
use crate::actor::message::MessageHandle;

// MessageInvoker trait
#[async_trait]
pub trait MessageInvoker: Debug + Send + Sync {
  async fn invoke_system_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError>;
  async fn invoke_user_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError>;
  async fn escalate_failure(&mut self, reason: ErrorReason, message_handle: MessageHandle);
  async fn record_mailbox_queue_latency(&mut self, _queue: MailboxQueueKind, _latency: Duration) {}
  async fn record_mailbox_queue_latency_snapshot(&mut self, _metrics: MailboxQueueLatencyMetrics) {}
  async fn record_mailbox_suspension_metrics(&mut self, _metrics: MailboxSuspensionMetrics, _suspended: bool) {}
  async fn record_mailbox_queue_length(&mut self, _queue: MailboxQueueKind, _length: u64) {}
}

#[derive(Debug, Clone)]
pub struct MessageInvokerHandle {
  inner: Arc<RwLock<dyn MessageInvoker>>,
  wants_metrics: Arc<AtomicBool>,
}

unsafe impl Send for MessageInvokerHandle {}
unsafe impl Sync for MessageInvokerHandle {}

impl MessageInvokerHandle {
  pub fn new(invoker: Arc<RwLock<dyn MessageInvoker>>) -> Self {
    Self::new_with_metrics(invoker, false)
  }

  pub fn new_with_metrics(invoker: Arc<RwLock<dyn MessageInvoker>>, wants_metrics: bool) -> Self {
    Self {
      inner: invoker,
      wants_metrics: Arc::new(AtomicBool::new(wants_metrics)),
    }
  }

  pub fn wants_metrics(&self) -> bool {
    self.wants_metrics.load(Ordering::Relaxed)
  }

  pub fn set_wants_metrics(&self, wants_metrics: bool) {
    self.wants_metrics.store(wants_metrics, Ordering::SeqCst);
  }
}

impl PartialEq for MessageInvokerHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Eq for MessageInvokerHandle {}

impl std::hash::Hash for MessageInvokerHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.inner.as_ref() as *const RwLock<dyn MessageInvoker>).hash(state);
  }
}

#[async_trait]
impl MessageInvoker for MessageInvokerHandle {
  async fn invoke_system_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.inner.write().await;
    mg.invoke_system_message(message_handle).await
  }

  async fn invoke_user_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.inner.write().await;
    mg.invoke_user_message(message_handle).await
  }

  async fn escalate_failure(&mut self, reason: ErrorReason, message_handle: MessageHandle) {
    let mut mg = self.inner.write().await;
    mg.escalate_failure(reason, message_handle).await;
  }

  async fn record_mailbox_queue_latency(&mut self, queue: MailboxQueueKind, latency: Duration) {
    let mut mg = self.inner.write().await;
    mg.record_mailbox_queue_latency(queue, latency).await;
  }

  async fn record_mailbox_queue_latency_snapshot(&mut self, metrics: MailboxQueueLatencyMetrics) {
    let mut mg = self.inner.write().await;
    mg.record_mailbox_queue_latency_snapshot(metrics).await;
  }

  async fn record_mailbox_suspension_metrics(&mut self, metrics: MailboxSuspensionMetrics, suspended: bool) {
    let mut mg = self.inner.write().await;
    mg.record_mailbox_suspension_metrics(metrics, suspended).await;
  }

  async fn record_mailbox_queue_length(&mut self, queue: MailboxQueueKind, length: u64) {
    let mut mg = self.inner.write().await;
    mg.record_mailbox_queue_length(queue, length).await;
  }
}

static_assertions::assert_impl_all!(MessageInvokerHandle: Send, Sync);
