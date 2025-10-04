use crate::actor::core::ActorError;
use crate::actor::core::ErrorReason;
use crate::actor::dispatch::bounded::BoundedMailboxQueue;
use crate::actor::dispatch::dispatcher::{CoreSchedulerDispatcher, Dispatcher, DispatcherHandle};
use crate::actor::dispatch::mailbox::core_queue_adapters::{
  PriorityCoreMailboxQueue, RingCoreMailboxQueue, UnboundedMpscCoreMailboxQueue,
};
use crate::actor::dispatch::mailbox::sync_queue_handles::SyncMailboxQueueHandles;
use crate::actor::dispatch::mailbox::{DefaultMailbox, MailboxHandle};
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::dispatch::unbounded::{unbounded_mpsc_mailbox_creator, UnboundedMailboxQueue};
use crate::actor::dispatch::{Mailbox, MailboxQueueKind};
use crate::actor::message::MessageHandle;
use crate::runtime::tokio_core_runtime;
use async_trait::async_trait;
use nexus_actor_core_rs::runtime::CoreSpawner as _;
use nexus_actor_core_rs::runtime::{AsyncYield, CoreTaskFuture};
use nexus_utils_std_rs::collections::{MpscUnboundedChannelQueue, PriorityQueue, QueueReader, QueueWriter, RingQueue};
use nexus_utils_std_rs::runtime::TokioCoreSpawner;
use parking_lot::Mutex;
use rand::prelude::*;
use rand::rngs::SmallRng;
use std::env;
use std::future::Future;
use std::pin::Pin;
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
async fn test_core_queue_handles_bridge_core_mailbox_queue() {
  let handles = SyncMailboxQueueHandles::new(MpscUnboundedChannelQueue::new());
  let core_queue = handles.core_queue();

  let payload = MessageHandle::new("payload".to_string());
  core_queue.offer(payload.clone()).unwrap();
  assert_eq!(core_queue.len(), 1);

  let polled = core_queue.poll().unwrap().unwrap();
  assert!(polled.is_typed::<String>());
  assert_eq!(core_queue.len(), 0);
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

  let dispatcher = CoreSchedulerDispatcher::from_runtime(tokio_core_runtime());

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

    let task: CoreTaskFuture = Box::pin(async move {
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
    let handle = TokioCoreSpawner::current().spawn(task).expect("spawn mailbox producer");
    join_handles.push(handle);
  }

  for h in join_handles {
    h.clone().join().await;
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

  let dispatcher = CoreSchedulerDispatcher::from_runtime(tokio_core_runtime());

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

    let task: CoreTaskFuture = Box::pin(async move {
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
    let handle = TokioCoreSpawner::current()
      .spawn(task)
      .expect("spawn mailbox system producer");
    join_handles.push(handle);
  }

  for h in join_handles {
    h.clone().join().await;
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

#[test]
fn test_bounded_mailbox() {
  let size = 3;
  let mut m = BoundedMailboxQueue::new(RingQueue::new(size), size, false);
  m.offer(MessageHandle::new("1".to_string())).unwrap();
  m.offer(MessageHandle::new("2".to_string())).unwrap();
  m.offer(MessageHandle::new("3".to_string())).unwrap();
  let result = m.poll().unwrap();
  let value = result.unwrap().to_typed::<String>().unwrap();
  assert_eq!(value, "1".to_string());
}

#[tokio::test]
async fn test_mailbox_suspension_metrics_accumulates_duration() {
  let user_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
  let system_queue = UnboundedMailboxQueue::new(MpscUnboundedChannelQueue::new());
  let mut mailbox = DefaultMailbox::new(user_queue, system_queue);

  let message_invoker = Arc::new(RwLock::new(TestMessageInvoker::new(usize::MAX)));
  let dispatcher = CoreSchedulerDispatcher::from_runtime(tokio_core_runtime());

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
  let dispatcher = CoreSchedulerDispatcher::from_runtime(tokio_core_runtime());

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
  let dispatcher = CoreSchedulerDispatcher::from_runtime(tokio_core_runtime());

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
  let dispatcher = CoreSchedulerDispatcher::from_runtime(tokio_core_runtime());

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

#[test]
fn test_bounded_dropping_mailbox() {
  let size = 3;
  let mut m = BoundedMailboxQueue::new(RingQueue::new(size), size, true);
  m.offer(MessageHandle::new("1".to_string())).unwrap();
  m.offer(MessageHandle::new("2".to_string())).unwrap();
  m.offer(MessageHandle::new("3".to_string())).unwrap();
  m.offer(MessageHandle::new("4".to_string())).unwrap();
  let result = m.poll().unwrap();
  let value = result.unwrap().to_typed::<String>().unwrap();
  assert_eq!(value, "2".to_string());
}

#[test]
fn test_core_queue_handles_reflect_bounded_drop() {
  let size = 3;
  let user_handles = SyncMailboxQueueHandles::new(BoundedMailboxQueue::new(RingQueue::new(size), size, true));
  let writer = user_handles.writer_handle();
  writer.offer_sync(MessageHandle::new("1".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("2".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("3".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("4".to_string())).unwrap();

  let core_user_queue = user_handles.core_queue();
  assert_eq!(core_user_queue.len(), size);

  let reader = user_handles.reader_handle();
  let next = reader.poll_sync().unwrap().unwrap();
  assert_eq!(next.to_typed::<String>().unwrap(), "2".to_string());
}

#[tokio::test]
async fn test_bounded_mailbox_metrics_remain_consistent_after_drop() {
  let size = 3;
  let ring_queue = RingQueue::new(size);
  let user_core = Arc::new(RingCoreMailboxQueue::new(ring_queue.clone()));
  let user_handles = SyncMailboxQueueHandles::new_with_core(
    BoundedMailboxQueue::new(ring_queue, size, true),
    Some(user_core.clone()),
  );

  let system_queue = MpscUnboundedChannelQueue::new();
  let system_core = Arc::new(UnboundedMpscCoreMailboxQueue::new(system_queue.clone()));
  let system_handles =
    SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(system_queue), Some(system_core));

  let mailbox = DefaultMailbox::from_handles(user_handles.clone(), system_handles);

  let writer = user_handles.writer_handle();
  writer.offer_sync(MessageHandle::new("1".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("2".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("3".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("4".to_string())).unwrap();

  let (core_user, core_system) = mailbox.core_queue_handles();
  assert_eq!(core_user.len(), size);
  assert_eq!(core_system.len(), 0);

  mailbox.test_record_queue_latency(MailboxQueueKind::User, Duration::from_millis(5));
  let latency_metrics = mailbox.queue_latency_metrics();
  assert_eq!(latency_metrics.total_samples(MailboxQueueKind::User), 1);

  let suspension = mailbox.suspension_metrics();
  assert_eq!(suspension.resume_events, 0);
  assert_eq!(suspension.total_duration, Duration::from_nanos(0));

  let sync_handle = mailbox.to_sync_handle();
  let (user_core_handle, system_core_handle) = sync_handle.core_queue_handles().expect("core handles");
  assert_eq!(user_core_handle.len(), size);
  assert_eq!(system_core_handle.len(), 0);
}

#[tokio::test]
async fn test_priority_mailbox_core_queue_len_consistency() {
  let priority_queue = PriorityQueue::new(|| RingQueue::new(10));
  let user_core = Arc::new(PriorityCoreMailboxQueue::new(priority_queue.clone()));
  let user_handles =
    SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(priority_queue), Some(user_core.clone()));

  let system_mpsc = MpscUnboundedChannelQueue::new();
  let system_core = Arc::new(UnboundedMpscCoreMailboxQueue::new(system_mpsc.clone()));
  let system_handles =
    SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(system_mpsc), Some(system_core));

  let mailbox = DefaultMailbox::from_handles(user_handles.clone(), system_handles);

  let writer = user_handles.writer_handle();
  writer.offer_sync(MessageHandle::new("low".to_string())).unwrap();
  writer.offer_sync(MessageHandle::new("high".to_string())).unwrap();

  let (core_user, core_system) = mailbox.core_queue_handles();
  assert_eq!(core_user.len(), 2);
  assert_eq!(core_system.len(), 0);

  let sync_handle = mailbox.to_sync_handle();
  let (user_core_handle, system_core_handle) = sync_handle.core_queue_handles().expect("core handles");
  assert_eq!(user_core_handle.len(), 2);
  assert_eq!(system_core_handle.len(), 0);
}

#[derive(Debug)]
struct TestYield {
  hits: Arc<AtomicUsize>,
}

impl TestYield {
  fn new(counter: Arc<AtomicUsize>) -> Self {
    Self { hits: counter }
  }
}

impl AsyncYield for TestYield {
  fn yield_now(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    self.hits.fetch_add(1, Ordering::SeqCst);
    Box::pin(async {})
  }
}

fn new_default_mailbox() -> DefaultMailbox<
  UnboundedMailboxQueue<RingQueue<MessageHandle>>,
  UnboundedMailboxQueue<MpscUnboundedChannelQueue<MessageHandle>>,
> {
  let ring_queue = RingQueue::new(4);
  let user_core = Arc::new(RingCoreMailboxQueue::new(ring_queue.clone()));
  let user_handles = SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(ring_queue), Some(user_core));

  let system_queue = MpscUnboundedChannelQueue::new();
  let system_core = Arc::new(UnboundedMpscCoreMailboxQueue::new(system_queue.clone()));
  let system_handles =
    SyncMailboxQueueHandles::new_with_core(UnboundedMailboxQueue::new(system_queue), Some(system_core));

  DefaultMailbox::from_handles(user_handles, system_handles)
}

#[tokio::test]
async fn default_mailbox_cooperative_yield_uses_async_yielder() {
  let mut mailbox = new_default_mailbox();
  let counter = Arc::new(AtomicUsize::new(0));
  let yielder = Arc::new(TestYield::new(counter.clone()));
  let yielder_dyn: Arc<dyn AsyncYield> = yielder.clone();

  mailbox.install_async_yielder(Some(yielder_dyn)).await;
  mailbox.cooperative_yield().await;

  assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn mailbox_handle_install_async_yielder_propagates_to_mailbox() {
  let mailbox = new_default_mailbox();
  let mut handle = MailboxHandle::new(mailbox.clone());
  let counter = Arc::new(AtomicUsize::new(0));
  let yielder = Arc::new(TestYield::new(counter.clone()));
  let yielder_dyn: Arc<dyn AsyncYield> = yielder.clone();

  handle.install_async_yielder(Some(yielder_dyn)).await;
  mailbox.cooperative_yield().await;

  assert_eq!(counter.load(Ordering::SeqCst), 1);
}
