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
use nexus_actor_utils_rs::collections::{MpscUnboundedChannelQueue, QueueReader, QueueWriter, RingQueue};
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
