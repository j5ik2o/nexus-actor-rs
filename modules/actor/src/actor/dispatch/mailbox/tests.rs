use crate::actor::core::ActorError;
use crate::actor::core::ErrorReason;
use crate::actor::dispatch::bounded::BoundedMailboxQueue;
use crate::actor::dispatch::dispatcher::{Dispatcher, DispatcherHandle, TokioRuntimeContextDispatcher};
use crate::actor::dispatch::mailbox::DefaultMailbox;
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::dispatch::unbounded::{unbounded_mpsc_mailbox_creator, UnboundedMailboxQueue};
use crate::actor::dispatch::{Mailbox, MailboxQueueKind};
use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use nexus_utils_std_rs::collections::{MpscUnboundedChannelQueue, QueueReader, QueueWriter, RingQueue};
use parking_lot::Mutex;
use rand::prelude::*;
use rand::rngs::SmallRng;
use std::env;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::{Notify, RwLock};
use tokio::time::{sleep, timeout};
use tracing_subscriber::EnvFilter;

struct EnvVarGuard {
  key: &'static str,
  original: Option<String>,
}

impl EnvVarGuard {
  fn set(key: &'static str, value: &str) -> Self {
    let original = env::var(key).ok();
    env::set_var(key, value);
    Self { key, original }
  }
}

impl Drop for EnvVarGuard {
  fn drop(&mut self) {
    if let Some(ref value) = self.original {
      env::set_var(self.key, value);
    } else {
      env::remove_var(self.key);
    }
  }
}

#[derive(Debug)]
struct TestMessageInvoker {
  count: usize,
  max: usize,
  assert_flg: bool,
}

impl TestMessageInvoker {
  fn new(max: usize) -> Self {
    Self {
      count: 0,
      max,
      assert_flg: false,
    }
  }

  fn get_count(&self) -> usize {
    self.count
  }

  fn is_assert_flg(&self) -> bool {
    self.assert_flg
  }
}

#[async_trait]
impl MessageInvoker for TestMessageInvoker {
  async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    self.count += 1;
    if self.count == self.max {
      self.assert_flg = true;
    }
    Ok(())
  }

  async fn invoke_user_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    self.count += 1;
    if self.count == self.max {
      self.assert_flg = true;
    }
    Ok(())
  }

  async fn escalate_failure(&mut self, _: ErrorReason, _: MessageHandle) {}

  async fn record_mailbox_queue_latency(&mut self, _: MailboxQueueKind, _: Duration) {}
}

#[derive(Debug, Clone)]
struct HintDispatcher {
  handle: Handle,
  throughput: i32,
  yield_on: Arc<AtomicBool>,
}

impl HintDispatcher {
  fn new(throughput: i32) -> Self {
    Self {
      handle: Handle::current(),
      throughput,
      yield_on: Arc::new(AtomicBool::new(false)),
    }
  }

  fn set_hint(&self, value: bool) {
    self.yield_on.store(value, Ordering::SeqCst);
  }
}

#[async_trait]
impl Dispatcher for HintDispatcher {
  async fn schedule(&self, runner: crate::actor::dispatch::dispatcher::Runnable) {
    let handle = self.handle.clone();
    handle.spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }

  fn yield_hint(&self) -> bool {
    self.yield_on.load(Ordering::SeqCst)
  }
}

#[derive(Debug)]
struct YieldAwareMessageInvoker {
  processed: Arc<AtomicUsize>,
  target: usize,
  notify: Arc<Notify>,
}

impl YieldAwareMessageInvoker {
  fn new(target: usize) -> (Self, Arc<AtomicUsize>, Arc<Notify>) {
    let processed = Arc::new(AtomicUsize::new(0));
    let notify = Arc::new(Notify::new());
    (
      Self {
        processed: processed.clone(),
        target,
        notify: notify.clone(),
      },
      processed,
      notify,
    )
  }
}

#[async_trait]
impl MessageInvoker for YieldAwareMessageInvoker {
  async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    let current = self.processed.fetch_add(1, Ordering::SeqCst) + 1;
    if current >= self.target {
      self.notify.notify_waiters();
    }
    Ok(())
  }

  async fn invoke_user_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    let current = self.processed.fetch_add(1, Ordering::SeqCst) + 1;
    if current >= self.target {
      self.notify.notify_waiters();
    }
    Ok(())
  }

  async fn escalate_failure(&mut self, _: ErrorReason, _: MessageHandle) {}

  async fn record_mailbox_queue_latency(&mut self, _: MailboxQueueKind, _: Duration) {}
}

#[derive(Debug)]
struct MetricsProbeInvoker {
  notify: Arc<Notify>,
  latency_calls: Arc<AtomicUsize>,
}

impl MetricsProbeInvoker {
  fn new(notify: Arc<Notify>, latency_calls: Arc<AtomicUsize>) -> Self {
    Self { notify, latency_calls }
  }
}

#[async_trait]
impl MessageInvoker for MetricsProbeInvoker {
  async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn invoke_user_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    self.notify.notify_waiters();
    Ok(())
  }

  async fn escalate_failure(&mut self, _: ErrorReason, _: MessageHandle) {}

  async fn record_mailbox_queue_latency(&mut self, _: MailboxQueueKind, _: Duration) {
    self.latency_calls.fetch_add(1, Ordering::SeqCst);
  }
}

#[derive(Debug)]
struct QueueLengthProbeInvoker {
  notify: Arc<Notify>,
  samples: Arc<Mutex<Vec<u64>>>,
  target: usize,
}

impl QueueLengthProbeInvoker {
  fn new(notify: Arc<Notify>, samples: Arc<Mutex<Vec<u64>>>, target: usize) -> Self {
    Self {
      notify,
      samples,
      target,
    }
  }
}

#[async_trait]
impl MessageInvoker for QueueLengthProbeInvoker {
  async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn invoke_user_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn escalate_failure(&mut self, _: ErrorReason, _: MessageHandle) {}

  async fn record_mailbox_queue_length(&mut self, queue: MailboxQueueKind, length: u64) {
    if !matches!(queue, MailboxQueueKind::User) {
      return;
    }
    let mut guard = self.samples.lock();
    guard.push(length);
    if guard.len() >= self.target {
      self.notify.notify_waiters();
    }
  }
}

#[tokio::test]
async fn test_unbounded_mpsc_mailbox_user_message_consistency() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let max = 100;
  let c = 10;

  let mbox_producer = unbounded_mpsc_mailbox_creator();
  let message_invoker = Arc::new(RwLock::new(TestMessageInvoker::new(max)));
  let mut mailbox = mbox_producer.run().await;

  let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(message_invoker.clone())),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;

  let mut join_handles = Vec::new();
  let rng = SmallRng::from_rng(&mut rand::rng());

  for j in 0..c {
    let cmax = max / c;
    let mailbox = mailbox.clone();
    let mut rng = rng.clone();

    let h = tokio::spawn(async move {
      for i in 0..cmax {
        if rng.random_range(0..10) == 0 {
          let wait_time = rng.random_range(0..1000);
          sleep(Duration::from_millis(wait_time)).await;
        }
        mailbox
          .post_user_message(MessageHandle::new(format!("{} {}", j, i)))
          .await;
      }
    });
    join_handles.push(h);
  }

  for h in join_handles {
    h.await.unwrap();
  }

  sleep(Duration::from_secs(1)).await;

  {
    let mg = message_invoker.read().await;
    assert_eq!(mg.get_count(), max);
    assert!(mg.is_assert_flg());
  }
}

#[tokio::test]
async fn test_unbounded_mpsc_mailbox_system_message_consistency() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let max = 100;
  let c = 10;

  let mbox_producer = unbounded_mpsc_mailbox_creator();
  let message_invoker = Arc::new(RwLock::new(TestMessageInvoker::new(max)));
  let mut mailbox = mbox_producer.run().await;

  let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(message_invoker.clone())),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;

  let mut join_handles = Vec::new();
  let rng = SmallRng::from_rng(&mut rand::rng());

  for j in 0..c {
    let cmax = max / c;
    let mailbox = mailbox.clone();
    let mut rng = rng.clone();

    let h = tokio::spawn(async move {
      for i in 0..cmax {
        if rng.random_range(0..10) == 0 {
          let wait_time = rng.random_range(0..1000);
          sleep(Duration::from_millis(wait_time)).await;
        }
        mailbox
          .post_system_message(MessageHandle::new(format!("{} {}", j, i)))
          .await;
      }
    });
    join_handles.push(h);
  }

  for h in join_handles {
    h.await.unwrap();
  }

  sleep(Duration::from_secs(1)).await;

  {
    let mg = message_invoker.read().await;
    assert_eq!(mg.get_count(), max);
    assert!(mg.is_assert_flg());
  }
}

#[tokio::test]
async fn test_mailbox_progress_with_dispatcher_yield_hint() {
  let target = 16;
  let mbox_producer = unbounded_mpsc_mailbox_creator();
  let (invoker_impl, processed_counter, notify) = YieldAwareMessageInvoker::new(target);
  let message_invoker = Arc::new(RwLock::new(invoker_impl));
  let mut mailbox = mbox_producer.run().await;

  let dispatcher = HintDispatcher::new(1);
  dispatcher.set_hint(true);

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(message_invoker.clone())),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;

  for i in 0..target {
    mailbox
      .post_user_message(MessageHandle::new(format!("hint-{}", i)))
      .await;
  }

  timeout(Duration::from_millis(500), notify.notified())
    .await
    .expect("mailbox did not process messages under yield_hint");

  assert_eq!(processed_counter.load(Ordering::SeqCst), target);

  dispatcher.set_hint(false);
}

#[tokio::test]
async fn test_bounded_mailbox() {
  let size = 3;
  let mut m = BoundedMailboxQueue::new(RingQueue::new(size), size, false);
  m.offer(MessageHandle::new("1".to_string())).await.unwrap();
  m.offer(MessageHandle::new("2".to_string())).await.unwrap();
  m.offer(MessageHandle::new("3".to_string())).await.unwrap();
  let result = m.poll().await.unwrap();
  let value = result.unwrap().to_typed::<String>().unwrap();
  assert_eq!(value, "1".to_string());
}

#[tokio::test]
async fn test_mailbox_suspension_metrics_accumulates_duration() {
  let user_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
  let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
  let mut mailbox = DefaultMailbox::new(user_queue, system_queue);

  let message_invoker = Arc::new(RwLock::new(TestMessageInvoker::new(usize::MAX)));
  let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(message_invoker)),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;

  mailbox
    .post_system_message(MessageHandle::new(MailboxMessage::SuspendMailbox))
    .await;
  sleep(Duration::from_millis(20)).await;
  mailbox
    .post_system_message(MessageHandle::new(MailboxMessage::ResumeMailbox))
    .await;

  for _ in 0..20 {
    let metrics = mailbox.suspension_metrics();
    if metrics.resume_events > 0 {
      assert!(metrics.resume_events >= 1);
      assert!(metrics.total_duration >= Duration::from_millis(15));
      if let Some(avg) = metrics.average_duration() {
        assert!(avg >= Duration::from_millis(15));
      }
      return;
    }
    sleep(Duration::from_millis(10)).await;
  }

  panic!("suspension metrics were not updated");
}

#[tokio::test]
async fn test_default_mailbox_skips_latency_metrics_without_interest() {
  let producer = unbounded_mpsc_mailbox_creator();
  let mut mailbox = producer.run().await;
  let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

  let notify = Arc::new(Notify::new());
  let latency_calls = Arc::new(AtomicUsize::new(0));
  let invoker = Arc::new(RwLock::new(MetricsProbeInvoker::new(
    notify.clone(),
    latency_calls.clone(),
  )));

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(invoker.clone())),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;
  mailbox.start().await;

  mailbox.post_user_message(MessageHandle::new("probe".to_string())).await;

  timeout(Duration::from_millis(500), notify.notified())
    .await
    .expect("message was not processed");

  assert_eq!(latency_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_default_mailbox_emits_latency_metrics_with_interest() {
  let _interval_guard = EnvVarGuard::set("MAILBOX_QUEUE_SNAPSHOT_INTERVAL", "1");
  let producer = unbounded_mpsc_mailbox_creator();
  let mut mailbox = producer.run().await;
  let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

  let notify = Arc::new(Notify::new());
  let latency_calls = Arc::new(AtomicUsize::new(0));
  let invoker = Arc::new(RwLock::new(MetricsProbeInvoker::new(
    notify.clone(),
    latency_calls.clone(),
  )));

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new_with_metrics(invoker.clone(), true)),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;
  mailbox.start().await;

  mailbox.post_user_message(MessageHandle::new("probe".to_string())).await;

  timeout(Duration::from_millis(500), notify.notified())
    .await
    .expect("message was not processed");

  assert!(latency_calls.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_default_mailbox_emits_queue_length_samples() {
  let _interval_guard = EnvVarGuard::set("MAILBOX_QUEUE_SNAPSHOT_INTERVAL", "2");
  let producer = unbounded_mpsc_mailbox_creator();
  let mut mailbox = producer.run().await;
  let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

  let notify = Arc::new(Notify::new());
  let samples = Arc::new(Mutex::new(Vec::new()));
  let invoker = Arc::new(RwLock::new(QueueLengthProbeInvoker::new(
    notify.clone(),
    samples.clone(),
    3,
  )));

  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new_with_metrics(invoker.clone(), true)),
      Some(DispatcherHandle::new(dispatcher.clone())),
    )
    .await;
  mailbox.start().await;

  mailbox.post_user_message(MessageHandle::new("first".to_string())).await;
  mailbox
    .post_user_message(MessageHandle::new("second".to_string()))
    .await;

  timeout(Duration::from_millis(500), notify.notified())
    .await
    .expect("queue length samples not observed");

  let collected = samples.lock().clone();
  assert!(
    collected.contains(&1),
    "expected queue length sample for 1, got {:?}",
    collected
  );
  assert!(
    collected.iter().any(|len| *len >= 2),
    "expected queue length sample >= 2, got {:?}",
    collected
  );
  assert!(
    collected.contains(&0),
    "expected queue length sample for 0, got {:?}",
    collected
  );
}

#[tokio::test]
async fn test_mailbox_queue_latency_histogram_percentiles() {
  let user_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
  let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
  let mailbox = DefaultMailbox::new(user_queue, system_queue);

  for i in 1..=10 {
    mailbox.test_record_queue_latency(MailboxQueueKind::User, Duration::from_millis((i * 10) as u64));
  }

  let metrics = mailbox.queue_latency_metrics();
  assert_eq!(metrics.total_samples(MailboxQueueKind::User), 10);
  let p50 = metrics
    .percentile(MailboxQueueKind::User, 50.0)
    .expect("p50 should exist");
  assert!(p50 >= Duration::from_millis(50));
}

#[tokio::test]
async fn test_mailbox_handle_exposes_sync_handle() {
  let producer = unbounded_mpsc_mailbox_creator();
  let mailbox = producer.run().await;
  let sync_handle = mailbox.sync_handle().expect("sync handle should exist");
  assert_eq!(sync_handle.user_messages_count(), 0);
  assert_eq!(sync_handle.system_messages_count(), 0);
  assert!(!sync_handle.is_suspended());
}

#[tokio::test]
async fn test_bounded_dropping_mailbox() {
  let size = 3;
  let mut m = BoundedMailboxQueue::new(RingQueue::new(size), size, true);
  m.offer(MessageHandle::new("1".to_string())).await.unwrap();
  m.offer(MessageHandle::new("2".to_string())).await.unwrap();
  m.offer(MessageHandle::new("3".to_string())).await.unwrap();
  m.offer(MessageHandle::new("4".to_string())).await.unwrap();
  let result = m.poll().await.unwrap();
  let value = result.unwrap().to_typed::<String>().unwrap();
  assert_eq!(value, "2".to_string());
}
