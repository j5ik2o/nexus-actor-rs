use std::collections::VecDeque;
use std::convert::TryFrom;
use std::env;
use std::fmt::{self, Debug, Formatter};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::actor::dispatch::dispatcher::{Dispatcher, DispatcherHandle, Runnable};
use crate::actor::dispatch::mailbox::mailbox_handle::MailboxHandle;
use crate::actor::dispatch::mailbox::sync_queue_handles::{
  QueueReaderHandle, QueueWriterHandle, SyncMailboxQueue, SyncMailboxQueueHandles,
};
use crate::actor::dispatch::mailbox::{Mailbox, MailboxQueueKind, MailboxSync, MailboxSyncHandle};
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::mailbox_middleware::{MailboxMiddleware, MailboxMiddlewareHandle};
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::message::MessageHandle;
use crate::runtime::runtime_yield_now;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use nexus_actor_core_rs::actor::core_types::mailbox::CoreMailboxQueue;
use nexus_actor_core_rs::runtime::AsyncYield;
use nexus_utils_std_rs::collections::QueueError;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub(crate) struct MailboxYieldConfig {
  pub system_burst_limit: usize,
  pub user_priority_ratio: i32,
  pub backlog_sensitivity: usize,
  pub queue_latency_snapshot_interval: usize,
}

impl Default for MailboxYieldConfig {
  fn default() -> Self {
    Self {
      system_burst_limit: 4,
      user_priority_ratio: 2,
      backlog_sensitivity: 2,
      queue_latency_snapshot_interval: 64,
    }
  }
}

#[derive(Debug, Clone)]
struct QueueLatencyTracker {
  timestamps: Arc<Mutex<VecDeque<Instant>>>,
  histogram: Arc<LatencyHistogram>,
}

impl Default for QueueLatencyTracker {
  fn default() -> Self {
    Self {
      timestamps: Arc::new(Mutex::new(VecDeque::new())),
      histogram: Arc::new(LatencyHistogram::new()),
    }
  }
}

impl QueueLatencyTracker {
  fn record_enqueue(&self) {
    let mut guard = self.timestamps.lock();
    guard.push_back(Instant::now());
  }

  fn record_dequeue(&self) -> Option<Duration> {
    let mut guard = self.timestamps.lock();
    guard.pop_front().map(|instant| {
      let elapsed = instant.elapsed();
      self.histogram.record(elapsed);
      elapsed
    })
  }

  fn clear(&self) {
    let mut guard = self.timestamps.lock();
    guard.clear();
  }

  fn snapshot(&self) -> LatencyHistogramSnapshot {
    self.histogram.snapshot()
  }

  #[cfg(test)]
  fn record_for_tests(&self, duration: Duration) {
    self.histogram.record(duration);
  }
}

#[derive(Debug, Clone, Copy)]
enum QueueLengthEvent {
  Enqueue,
  Dequeue,
}

const SMALL_QUEUE_LENGTH_THRESHOLD: u64 = 4;

#[derive(Debug)]
struct LatencyHistogram {
  counts: Box<[AtomicU64]>,
  total: AtomicU64,
}

#[derive(Debug, Clone)]
pub(crate) struct LatencyHistogramSnapshot {
  counts: Vec<u64>,
  total: u64,
}

impl Default for LatencyHistogramSnapshot {
  fn default() -> Self {
    Self {
      counts: vec![0; LATENCY_BUCKET_BOUNDS.len() + 1],
      total: 0,
    }
  }
}

impl LatencyHistogram {
  fn new() -> Self {
    let counts = (0..=LATENCY_BUCKET_BOUNDS.len())
      .map(|_| AtomicU64::new(0))
      .collect::<Vec<_>>()
      .into_boxed_slice();
    Self {
      counts,
      total: AtomicU64::new(0),
    }
  }

  fn record(&self, duration: Duration) {
    let nanos = duration.as_nanos().min(u64::MAX as u128) as u64;
    let idx = match LATENCY_BUCKET_BOUNDS.iter().position(|&bound| nanos <= bound) {
      Some(i) => i,
      None => LATENCY_BUCKET_BOUNDS.len(),
    };
    self.counts[idx].fetch_add(1, Ordering::Relaxed);
    self.total.fetch_add(1, Ordering::Relaxed);
  }

  fn snapshot(&self) -> LatencyHistogramSnapshot {
    let counts = self
      .counts
      .iter()
      .map(|counter| counter.load(Ordering::Relaxed))
      .collect::<Vec<_>>();
    let total = self.total.load(Ordering::Relaxed);
    LatencyHistogramSnapshot { counts, total }
  }
}

impl LatencyHistogramSnapshot {
  pub fn total(&self) -> u64 {
    self.total
  }

  pub fn counts(&self) -> &[u64] {
    &self.counts
  }

  pub fn percentile(&self, percentile: f64) -> Option<Duration> {
    if self.total == 0 {
      return None;
    }
    let p = percentile.clamp(0.0, 100.0);
    let target = ((p / 100.0) * self.total as f64).ceil().max(1.0) as u64;
    let mut accum = 0u64;
    for (idx, bucket) in self.counts.iter().enumerate() {
      accum = accum.saturating_add(*bucket);
      if accum >= target {
        return Some(duration_for_bucket(idx));
      }
    }
    Some(duration_for_bucket(self.counts.len() - 1))
  }
}

const LATENCY_BUCKET_BOUNDS: [u64; 17] = [
  1_000,
  5_000,
  10_000,
  25_000,
  50_000,
  100_000,
  250_000,
  500_000,
  1_000_000,
  2_500_000,
  5_000_000,
  10_000_000,
  25_000_000,
  50_000_000,
  100_000_000,
  250_000_000,
  500_000_000,
];

fn duration_for_bucket(index: usize) -> Duration {
  if index < LATENCY_BUCKET_BOUNDS.len() {
    Duration::from_nanos(LATENCY_BUCKET_BOUNDS[index])
  } else {
    let base = Duration::from_nanos(LATENCY_BUCKET_BOUNDS[LATENCY_BUCKET_BOUNDS.len() - 1]);
    base.checked_mul(2).unwrap_or(base)
  }
}

#[derive(Debug, Default)]
struct MailboxSuspensionState {
  flag: AtomicBool,
  since: Mutex<Option<Instant>>,
  total_nanos: AtomicU64,
  resume_events: AtomicU64,
}

impl MailboxSuspensionState {
  fn set(&self, suspended: bool) {
    let was = self.flag.swap(suspended, Ordering::SeqCst);
    if suspended {
      if !was {
        let mut guard = self.since.lock();
        *guard = Some(Instant::now());
      }
    } else if was {
      let mut guard = self.since.lock();
      if let Some(started) = guard.take() {
        let duration = started.elapsed();
        self.record_resume(duration);
      }
    }
  }

  fn is_suspended(&self) -> bool {
    self.flag.load(Ordering::SeqCst)
  }

  #[allow(dead_code)]
  fn duration(&self) -> Option<Duration> {
    let guard = self.since.lock();
    guard.map(|since| since.elapsed())
  }

  fn record_resume(&self, duration: Duration) {
    let nanos = duration.as_nanos();
    let nanos = nanos.min(u64::MAX as u128) as u64;
    self.total_nanos.fetch_add(nanos, Ordering::SeqCst);
    self.resume_events.fetch_add(1, Ordering::SeqCst);
  }

  fn metrics(&self) -> MailboxSuspensionMetrics {
    let nanos = self.total_nanos.load(Ordering::SeqCst);
    let events = self.resume_events.load(Ordering::SeqCst);
    MailboxSuspensionMetrics {
      resume_events: events,
      total_duration: Duration::from_nanos(nanos),
    }
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MailboxSuspensionMetrics {
  pub resume_events: u64,
  pub total_duration: Duration,
}

impl MailboxSuspensionMetrics {
  pub fn average_duration(&self) -> Option<Duration> {
    if self.resume_events == 0 {
      return None;
    }
    let nanos = self.total_duration.as_nanos();
    let avg = nanos / self.resume_events as u128;
    Some(Duration::from_nanos(avg.min(u64::MAX as u128) as u64))
  }
}

#[derive(Debug, Clone)]
pub struct MailboxQueueLatencyMetrics {
  user: LatencyHistogramSnapshot,
  system: LatencyHistogramSnapshot,
}

impl Default for MailboxQueueLatencyMetrics {
  fn default() -> Self {
    Self {
      user: LatencyHistogramSnapshot::default(),
      system: LatencyHistogramSnapshot::default(),
    }
  }
}

impl MailboxQueueLatencyMetrics {
  pub fn percentile(&self, queue: MailboxQueueKind, percentile: f64) -> Option<Duration> {
    match queue {
      MailboxQueueKind::User => self.user.percentile(percentile),
      MailboxQueueKind::System => self.system.percentile(percentile),
    }
  }

  pub fn total_samples(&self, queue: MailboxQueueKind) -> u64 {
    match queue {
      MailboxQueueKind::User => self.user.total(),
      MailboxQueueKind::System => self.system.total(),
    }
  }

  pub fn bucket_counts(&self, queue: MailboxQueueKind) -> &[u64] {
    match queue {
      MailboxQueueKind::User => self.user.counts(),
      MailboxQueueKind::System => self.system.counts(),
    }
  }
}

#[derive(Debug)]
pub(crate) struct DefaultMailboxInner<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue, {
  user_mailbox_writer: QueueWriterHandle<UQ>,
  user_mailbox_reader: QueueReaderHandle<UQ>,
  system_mailbox_writer: QueueWriterHandle<SQ>,
  system_mailbox_reader: QueueReaderHandle<SQ>,
  middlewares: Vec<MailboxMiddlewareHandle>,
}

// DefaultMailbox implementation
#[derive(Clone)]
pub(crate) struct DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue, {
  inner: Arc<Mutex<DefaultMailboxInner<UQ, SQ>>>,
  scheduler_status: Arc<AtomicBool>,
  user_messages_count: Arc<AtomicI32>,
  system_messages_count: Arc<AtomicI32>,
  suspension: Arc<MailboxSuspensionState>,
  last_backlog: Arc<AtomicI32>,
  invoker_opt: Arc<ArcSwapOption<MessageInvokerHandle>>,
  dispatcher_opt: Arc<ArcSwapOption<DispatcherHandle>>,
  async_yielder: Arc<Mutex<Option<Arc<dyn AsyncYield>>>>,
  config: MailboxYieldConfig,
  user_queue_latency: QueueLatencyTracker,
  system_queue_latency: QueueLatencyTracker,
  metrics_enabled: Arc<AtomicBool>,
  user_snapshot_counter: Arc<AtomicU64>,
  system_snapshot_counter: Arc<AtomicU64>,
  core_user_queue: Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
  core_system_queue: Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
}

impl<UQ, SQ> Debug for DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("DefaultMailbox").finish_non_exhaustive()
  }
}

impl<UQ, SQ> DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue,
{
  pub(crate) fn new(user_mailbox: UQ, system_mailbox: SQ) -> Self {
    let user_handles = SyncMailboxQueueHandles::new(user_mailbox);
    let system_handles = SyncMailboxQueueHandles::new(system_mailbox);
    Self::from_handles(user_handles, system_handles)
  }

  pub(crate) fn from_handles(
    user_handles: SyncMailboxQueueHandles<UQ>,
    system_handles: SyncMailboxQueueHandles<SQ>,
  ) -> Self {
    let scheduler_status = Arc::new(AtomicBool::new(false));
    let user_messages_count = Arc::new(AtomicI32::new(0));
    let system_messages_count = Arc::new(AtomicI32::new(0));
    let suspension = Arc::new(MailboxSuspensionState::default());
    let last_backlog = Arc::new(AtomicI32::new(0));
    let mut config = MailboxYieldConfig::default();
    if let Ok(value) = env::var("MAILBOX_QUEUE_SNAPSHOT_INTERVAL") {
      if let Ok(interval) = value.parse::<usize>() {
        config.queue_latency_snapshot_interval = interval.max(1);
      }
    }

    let user_queue_latency = QueueLatencyTracker::default();
    let system_queue_latency = QueueLatencyTracker::default();

    Self {
      inner: Arc::new(Mutex::new(DefaultMailboxInner {
        user_mailbox_writer: user_handles.writer_handle(),
        user_mailbox_reader: user_handles.reader_handle(),
        system_mailbox_writer: system_handles.writer_handle(),
        system_mailbox_reader: system_handles.reader_handle(),
        middlewares: vec![],
      })),
      scheduler_status,
      user_messages_count,
      system_messages_count,
      suspension,
      last_backlog,
      invoker_opt: Arc::new(ArcSwapOption::from(None)),
      dispatcher_opt: Arc::new(ArcSwapOption::from(None)),
      async_yielder: Arc::new(Mutex::new(None)),
      config,
      user_queue_latency,
      system_queue_latency,
      metrics_enabled: Arc::new(AtomicBool::new(false)),
      user_snapshot_counter: Arc::new(AtomicU64::new(0)),
      system_snapshot_counter: Arc::new(AtomicU64::new(0)),
      core_user_queue: user_handles.core_queue(),
      core_system_queue: system_handles.core_queue(),
    }
  }

  pub(crate) async fn with_middlewares(self, middlewares: impl IntoIterator<Item = MailboxMiddlewareHandle>) -> Self {
    {
      let mut inner_mg = self.inner.lock();
      inner_mg.middlewares = middlewares.into_iter().collect();
    }
    self
  }

  pub(crate) fn with_yield_config(mut self, config: MailboxYieldConfig) -> Self {
    self.config = config;
    self
  }

  pub(crate) fn with_latency_snapshot_interval(mut self, interval: usize) -> Self {
    self.config.queue_latency_snapshot_interval = interval.max(1);
    self
  }

  pub(crate) fn suspension_metrics(&self) -> MailboxSuspensionMetrics {
    self.suspension.metrics()
  }

  pub(crate) fn to_sync_handle(&self) -> MailboxSyncHandle {
    MailboxSyncHandle::new(self.clone())
  }

  pub(crate) fn core_queue_handles(
    &self,
  ) -> (
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
  ) {
    (self.core_user_queue.clone(), self.core_system_queue.clone())
  }

  pub(crate) fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics {
    let user_snapshot = self.user_queue_latency.snapshot();
    let system_snapshot = self.system_queue_latency.snapshot();
    MailboxQueueLatencyMetrics {
      user: user_snapshot,
      system: system_snapshot,
    }
  }

  #[cfg(test)]
  pub(crate) fn test_record_queue_latency(&self, queue: MailboxQueueKind, duration: Duration) {
    match queue {
      MailboxQueueKind::User => self.user_queue_latency.record_for_tests(duration),
      MailboxQueueKind::System => self.system_queue_latency.record_for_tests(duration),
    }
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

  fn set_async_yielder(&self, yielder: Option<Arc<dyn AsyncYield>>) {
    let mut guard = self.async_yielder.lock();
    *guard = yielder;
  }

  pub(crate) async fn cooperative_yield(&self) {
    let yielder = { self.async_yielder.lock().clone() };
    if let Some(yielder) = yielder {
      yielder.yield_now().await;
    } else {
      runtime_yield_now().await;
    }
  }

  fn initialize_scheduler_status(&self) {
    self.scheduler_status.store(false, Ordering::SeqCst);
  }

  fn compare_exchange_scheduler_status(&self, current: bool, new: bool) -> Result<bool, bool> {
    self
      .scheduler_status
      .compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst)
  }

  fn metrics_enabled(&self) -> bool {
    self.metrics_enabled.load(Ordering::Relaxed)
  }

  fn queue_length(&self, queue: MailboxQueueKind) -> u64 {
    match queue {
      MailboxQueueKind::User => self.core_user_queue.len() as u64,
      MailboxQueueKind::System => self.core_system_queue.len() as u64,
    }
  }

  fn should_emit_queue_length(&self, event: QueueLengthEvent, length: u64) -> bool {
    let interval = self.config.queue_latency_snapshot_interval.max(1) as u64;
    match event {
      QueueLengthEvent::Enqueue => {
        if length == 0 {
          return false;
        }
        if length <= SMALL_QUEUE_LENGTH_THRESHOLD {
          return true;
        }
        length % interval == 0
      }
      QueueLengthEvent::Dequeue => {
        if length == 0 {
          return true;
        }
        if length < SMALL_QUEUE_LENGTH_THRESHOLD {
          return true;
        }
        (length + 1) % interval == 0
      }
    }
  }

  async fn maybe_emit_queue_length(&self, queue: MailboxQueueKind, event: QueueLengthEvent) {
    if !self.metrics_enabled() {
      return;
    }
    let Some(mut invoker) = self.message_invoker_opt() else {
      return;
    };
    let length = self.queue_length(queue);
    if !self.should_emit_queue_length(event, length) {
      return;
    }
    invoker.record_mailbox_queue_length(queue, length).await;
  }

  fn record_queue_enqueue(&self, queue: MailboxQueueKind) {
    if !self.metrics_enabled() {
      return;
    }
    match queue {
      MailboxQueueKind::User => self.user_queue_latency.record_enqueue(),
      MailboxQueueKind::System => self.system_queue_latency.record_enqueue(),
    }
  }

  fn record_queue_dequeue(&self, queue: MailboxQueueKind) -> Option<Duration> {
    if !self.metrics_enabled() {
      return None;
    }
    match queue {
      MailboxQueueKind::User => self.user_queue_latency.record_dequeue(),
      MailboxQueueKind::System => self.system_queue_latency.record_dequeue(),
    }
  }

  fn clear_queue_latency(&self, queue: MailboxQueueKind) {
    match queue {
      MailboxQueueKind::User => self.user_queue_latency.clear(),
      MailboxQueueKind::System => self.system_queue_latency.clear(),
    }
  }

  fn set_suspended(&self, suspended: bool) {
    self.suspension.set(suspended);
  }

  fn is_suspended(&self) -> bool {
    self.suspension.is_suspended()
  }

  fn increment_system_messages_count(&self) {
    self.system_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  fn decrement_system_messages_count(&self) {
    self.system_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  fn increment_user_messages_count(&self) {
    self.user_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  fn decrement_user_messages_count(&self) {
    self.user_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  fn middlewares(&self) -> Vec<MailboxMiddlewareHandle> {
    let inner_mg = self.inner.lock();
    inner_mg.middlewares.clone()
  }

  fn poll_system_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let receiver = {
      let inner_mg = self.inner.lock();
      inner_mg.system_mailbox_reader.clone()
    };
    match receiver.poll_sync() {
      Ok(message) => Ok(message),
      Err(err) => {
        if matches!(err, QueueError::PoolError) {
          self.clear_queue_latency(MailboxQueueKind::System);
        }
        Err(err)
      }
    }
  }

  fn poll_user_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let receiver = {
      let inner_mg = self.inner.lock();
      inner_mg.user_mailbox_reader.clone()
    };
    match receiver.poll_sync() {
      Ok(message) => Ok(message),
      Err(err) => {
        if matches!(err, QueueError::PoolError) {
          self.clear_queue_latency(MailboxQueueKind::User);
        }
        Err(err)
      }
    }
  }

  fn offer_system_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let sender = {
      let inner_mg = self.inner.lock();
      inner_mg.system_mailbox_writer.clone()
    };
    sender.offer_sync(element)
  }

  fn offer_user_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let sender = {
      let inner_mg = self.inner.lock();
      inner_mg.user_mailbox_writer.clone()
    };
    sender.offer_sync(element)
  }

  fn should_yield(
    &self,
    iteration: usize,
    throughput: i32,
    system_pending: i32,
    user_pending: i32,
    dispatcher_hint: bool,
  ) -> bool {
    if dispatcher_hint && iteration > 0 {
      return true;
    }
    if throughput <= 0 {
      return false;
    }
    let limit = throughput.max(1) as usize;
    if iteration < limit {
      return false;
    }

    let system_pending = system_pending.max(0);
    let user_pending = user_pending.max(0);
    let backlog = system_pending.saturating_add(user_pending);
    let previous = self.last_backlog.swap(backlog, Ordering::SeqCst);

    if backlog == 0 {
      return true;
    }

    let sensitivity = i32::try_from(self.config.backlog_sensitivity.max(1)).unwrap_or(i32::MAX);
    let backlog_delta = backlog.saturating_sub(previous.max(0));
    if backlog_delta >= sensitivity {
      let half_limit = if limit > 1 { limit / 2 } else { 1 };
      if iteration >= half_limit {
        return true;
      }
    }

    true
  }

  async fn try_handle_system_message(
    &self,
    message_invoker: &mut MessageInvokerHandle,
  ) -> Result<bool, QueueError<MessageHandle>> {
    match self.poll_system_mailbox() {
      Ok(Some(msg)) => {
        let mut emit_snapshot = false;
        if let Some(latency) = self.record_queue_dequeue(MailboxQueueKind::System) {
          let emit_update = self.should_emit_latency_update(MailboxQueueKind::System);
          if emit_update {
            message_invoker
              .record_mailbox_queue_latency(MailboxQueueKind::System, latency)
              .await;
          }
          emit_snapshot = emit_update;
        }
        self.decrement_system_messages_count();
        self
          .maybe_emit_queue_length(MailboxQueueKind::System, QueueLengthEvent::Dequeue)
          .await;
        if emit_snapshot {
          message_invoker
            .record_mailbox_queue_latency_snapshot(self.queue_latency_metrics())
            .await;
        }
        let mailbox_message = msg.to_typed::<MailboxMessage>();
        match mailbox_message {
          Some(MailboxMessage::SuspendMailbox) => {
            self.set_suspended(true);
            if self.metrics_enabled() {
              message_invoker
                .record_mailbox_suspension_metrics(self.suspension.metrics(), true)
                .await;
            }
          }
          Some(MailboxMessage::ResumeMailbox) => {
            self.set_suspended(false);
            if self.metrics_enabled() {
              message_invoker
                .record_mailbox_suspension_metrics(self.suspension.metrics(), false)
                .await;
            }
          }
          _ => {
            if let Err(err) = message_invoker.invoke_system_message(msg.clone()).await {
              message_invoker
                .escalate_failure(err.reason().cloned().unwrap(), msg.clone())
                .await;
            }
          }
        }
        let mut middlewares = self.middlewares();
        for middleware in &mut middlewares {
          middleware.message_received(&msg).await;
        }
        Ok(true)
      }
      Ok(None) => Ok(false),
      Err(err) => {
        if matches!(err, QueueError::PoolError) {
          self.clear_queue_latency(MailboxQueueKind::System);
        }
        Err(err)
      }
    }
  }

  async fn try_handle_user_message(
    &self,
    message_invoker: &mut MessageInvokerHandle,
  ) -> Result<bool, QueueError<MessageHandle>> {
    match self.poll_user_mailbox() {
      Ok(Some(message)) => {
        let mut emit_snapshot = false;
        if let Some(latency) = self.record_queue_dequeue(MailboxQueueKind::User) {
          let emit_update = self.should_emit_latency_update(MailboxQueueKind::User);
          if emit_update {
            message_invoker
              .record_mailbox_queue_latency(MailboxQueueKind::User, latency)
              .await;
          }
          emit_snapshot = emit_update;
        }
        self.decrement_user_messages_count();
        self
          .maybe_emit_queue_length(MailboxQueueKind::User, QueueLengthEvent::Dequeue)
          .await;
        if emit_snapshot {
          message_invoker
            .record_mailbox_queue_latency_snapshot(self.queue_latency_metrics())
            .await;
        }
        let result = message_invoker.invoke_user_message(message.clone()).await;
        if let Err(e) = result {
          message_invoker
            .escalate_failure(e.reason().cloned().unwrap(), message.clone())
            .await;
        }
        let mut middlewares = self.middlewares();
        for middleware in &mut middlewares {
          middleware.message_received(&message).await;
        }
        Ok(true)
      }
      Ok(None) => Ok(false),
      Err(err) => {
        if matches!(err, QueueError::PoolError) {
          self.clear_queue_latency(MailboxQueueKind::User);
        }
        Err(err)
      }
    }
  }

  async fn schedule(&self) {
    if self.compare_exchange_scheduler_status(false, true).is_ok() {
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
    let mut iteration = 0;
    let mut system_burst = 0;

    if self.dispatcher_opt().is_none() || self.message_invoker_opt().is_none() {
      return;
    }

    let dispatcher = self.dispatcher_opt().expect("Dispatcher is not set");
    let mut message_invoker = self.message_invoker_opt().expect("Message invoker is not set");

    let throughput = dispatcher.throughput().await;
    let system_burst_limit = self.config.system_burst_limit.max(1);
    let user_priority_ratio = self.config.user_priority_ratio.max(1);

    loop {
      let system_pending = self.system_messages_count.load(Ordering::SeqCst).max(0);
      let user_pending = self.user_messages_count.load(Ordering::SeqCst).max(0);
      let dispatcher_hint = dispatcher.yield_hint();

      if self.should_yield(iteration, throughput, system_pending, user_pending, dispatcher_hint) {
        iteration = 0;
        system_burst = 0;
        self.cooperative_yield().await;
        continue;
      }

      let user_has_priority = user_pending > system_pending * user_priority_ratio;

      if system_pending > 0
        && !user_has_priority
        && (system_burst < system_burst_limit || user_pending == 0)
        && self
          .try_handle_system_message(&mut message_invoker)
          .await
          .unwrap_or(false)
      {
        iteration += 1;
        system_burst += 1;
        continue;
      }

      if self.is_suspended() {
        break;
      }

      if user_pending > 0
        && self
          .try_handle_user_message(&mut message_invoker)
          .await
          .unwrap_or(false)
      {
        iteration += 1;
        system_burst = 0;
        continue;
      }

      if self
        .try_handle_system_message(&mut message_invoker)
        .await
        .unwrap_or(false)
      {
        iteration += 1;
        system_burst = 1;
        continue;
      }

      break;
    }
  }
}

impl<UQ, SQ> MailboxSync for DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue,
{
  fn user_messages_count(&self) -> i32 {
    self.user_messages_count.load(Ordering::SeqCst)
  }

  fn system_messages_count(&self) -> i32 {
    self.system_messages_count.load(Ordering::SeqCst)
  }

  fn suspension_metrics(&self) -> MailboxSuspensionMetrics {
    self.suspension_metrics()
  }

  fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics {
    self.queue_latency_metrics()
  }

  fn is_suspended(&self) -> bool {
    self.is_suspended()
  }

  fn core_queue_handles(
    &self,
  ) -> Option<(
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
    Arc<dyn CoreMailboxQueue<Error = QueueError<MessageHandle>> + Send + Sync>,
  )> {
    Some((self.core_user_queue.clone(), self.core_system_queue.clone()))
  }
}
impl<UQ, SQ> DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue,
{
  fn should_emit_latency_update(&self, queue: MailboxQueueKind) -> bool {
    if !self.metrics_enabled() {
      return false;
    }
    let interval = self.config.queue_latency_snapshot_interval.max(1) as u64;
    match queue {
      MailboxQueueKind::User => {
        let prev = self.user_snapshot_counter.fetch_add(1, Ordering::Relaxed);
        prev == 0 || (prev + 1) % interval == 0
      }
      MailboxQueueKind::System => {
        let prev = self.system_snapshot_counter.fetch_add(1, Ordering::Relaxed);
        prev == 0 || (prev + 1) % interval == 0
      }
    }
  }
}

#[async_trait]
impl<UQ, SQ> Mailbox for DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue,
{
  async fn get_user_messages_count(&self) -> i32 {
    self.user_messages_count.load(Ordering::SeqCst)
  }

  async fn get_system_messages_count(&self) -> i32 {
    self.system_messages_count.load(Ordering::SeqCst)
  }

  async fn process_messages(&self) {
    loop {
      self.run().await;

      self.initialize_scheduler_status();
      let system_messages_count = self.get_system_messages_count().await;
      let user_messages_count = self.get_user_messages_count().await;

      if (system_messages_count > 0 || (!self.is_suspended() && user_messages_count > 0))
        && self.compare_exchange_scheduler_status(false, true).is_ok()
      {
        continue;
      }

      // if system_messages_count > 0 || (!self.is_suspended() && user_messages_count > 0) {
      //   if self.compare_exchange_scheduler_status(false, true).is_ok() {
      //     continue;
      //   }
      // }

      break;
    }

    for mut middleware in self.middlewares() {
      middleware.mailbox_empty().await;
    }
  }

  async fn post_user_message(&self, message_handle: MessageHandle) {
    for mut middleware in self.middlewares() {
      middleware.message_posted(&message_handle).await;
    }

    if let Err(e) = self.offer_user_mailbox(message_handle) {
      tracing::error!("Failed to send message: {:?}", e);
    } else {
      self.increment_user_messages_count();
      self.record_queue_enqueue(MailboxQueueKind::User);
      self
        .maybe_emit_queue_length(MailboxQueueKind::User, QueueLengthEvent::Enqueue)
        .await;
      self.schedule().await;
    }
  }

  async fn post_system_message(&self, message_handle: MessageHandle) {
    for mut middleware in self.middlewares() {
      middleware.message_posted(&message_handle).await;
    }

    if let Err(e) = self.offer_system_mailbox(message_handle) {
      tracing::error!("Failed to send message: {:?}", e);
    } else {
      self.increment_system_messages_count();
      self.record_queue_enqueue(MailboxQueueKind::System);
      self
        .maybe_emit_queue_length(MailboxQueueKind::System, QueueLengthEvent::Enqueue)
        .await;
      self.schedule().await;
    }
  }

  async fn register_handlers(
    &mut self,
    message_invoker_handle: Option<MessageInvokerHandle>,
    dispatcher_handle: Option<DispatcherHandle>,
  ) {
    let metrics_enabled = message_invoker_handle
      .as_ref()
      .map(|handle| handle.wants_metrics())
      .unwrap_or(false);
    if let Some(handle) = message_invoker_handle.as_ref() {
      handle.set_wants_metrics(metrics_enabled);
    }
    self.metrics_enabled.store(metrics_enabled, Ordering::SeqCst);
    self.user_snapshot_counter.store(0, Ordering::Relaxed);
    self.system_snapshot_counter.store(0, Ordering::Relaxed);
    if !metrics_enabled {
      self.user_queue_latency.clear();
      self.system_queue_latency.clear();
    }
    self.set_message_invoker_opt(message_invoker_handle);
    self.set_dispatcher_opt(dispatcher_handle);
  }

  async fn start(&self) {
    for mut middleware in self.middlewares() {
      middleware.mailbox_started().await;
    }
  }

  async fn user_message_count(&self) -> i32 {
    self.get_user_messages_count().await
  }

  async fn install_async_yielder(&mut self, yielder: Option<Arc<dyn AsyncYield>>) {
    self.set_async_yielder(yielder);
  }

  async fn to_handle(&self) -> MailboxHandle {
    let sync = self.to_sync_handle();
    MailboxHandle::new_with_sync(self.clone(), Some(sync))
  }
}
