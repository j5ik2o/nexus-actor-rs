use std::collections::VecDeque;
use std::convert::TryFrom;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::actor::dispatch::dispatcher::{Dispatcher, DispatcherHandle, Runnable};
use crate::actor::dispatch::mailbox::mailbox_handle::MailboxHandle;
use crate::actor::dispatch::mailbox::sync_queue_handles::{
  SyncMailboxQueue, SyncMailboxQueueHandles, SyncQueueReaderHandle, SyncQueueWriterHandle,
};
use crate::actor::dispatch::mailbox::Mailbox;
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::mailbox_middleware::{MailboxMiddleware, MailboxMiddlewareHandle};
use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use crate::actor::dispatch::MailboxQueueKind;
use crate::actor::message::MessageHandle;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use nexus_actor_utils_rs::collections::QueueError;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub(crate) struct MailboxYieldConfig {
  pub system_burst_limit: usize,
  pub user_priority_ratio: i32,
  pub backlog_sensitivity: usize,
}

impl Default for MailboxYieldConfig {
  fn default() -> Self {
    Self {
      system_burst_limit: 4,
      user_priority_ratio: 2,
      backlog_sensitivity: 2,
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
pub(crate) struct MailboxSuspensionMetrics {
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
}

#[derive(Debug)]
pub(crate) struct DefaultMailboxInner<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue, {
  user_mailbox_writer: SyncQueueWriterHandle<UQ>,
  user_mailbox_reader: SyncQueueReaderHandle<UQ>,
  system_mailbox_writer: SyncQueueWriterHandle<SQ>,
  system_mailbox_reader: SyncQueueReaderHandle<SQ>,
  middlewares: Vec<MailboxMiddlewareHandle>,
  user_queue_latency: QueueLatencyTracker,
  system_queue_latency: QueueLatencyTracker,
}

// DefaultMailbox implementation
#[derive(Debug, Clone)]
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
  config: MailboxYieldConfig,
}

impl<UQ, SQ> DefaultMailbox<UQ, SQ>
where
  UQ: SyncMailboxQueue,
  SQ: SyncMailboxQueue,
{
  pub(crate) fn new(user_mailbox: UQ, system_mailbox: SQ) -> Self {
    let user_handles = SyncMailboxQueueHandles::new(user_mailbox);
    let system_handles = SyncMailboxQueueHandles::new(system_mailbox);
    let scheduler_status = Arc::new(AtomicBool::new(false));
    let user_messages_count = Arc::new(AtomicI32::new(0));
    let system_messages_count = Arc::new(AtomicI32::new(0));
    let suspension = Arc::new(MailboxSuspensionState::default());
    let last_backlog = Arc::new(AtomicI32::new(0));
    let config = MailboxYieldConfig::default();

    Self {
      inner: Arc::new(Mutex::new(DefaultMailboxInner {
        user_mailbox_writer: user_handles.writer_handle(),
        user_mailbox_reader: user_handles.reader_handle(),
        system_mailbox_writer: system_handles.writer_handle(),
        system_mailbox_reader: system_handles.reader_handle(),
        middlewares: vec![],
        user_queue_latency: QueueLatencyTracker::default(),
        system_queue_latency: QueueLatencyTracker::default(),
      })),
      scheduler_status,
      user_messages_count,
      system_messages_count,
      suspension,
      last_backlog,
      invoker_opt: Arc::new(ArcSwapOption::from(None)),
      dispatcher_opt: Arc::new(ArcSwapOption::from(None)),
      config,
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

  pub(crate) fn suspension_metrics(&self) -> MailboxSuspensionMetrics {
    self.suspension.metrics()
  }

  pub(crate) fn queue_latency_metrics(&self) -> MailboxQueueLatencyMetrics {
    let (user_snapshot, system_snapshot) = {
      let inner_mg = self.inner.lock();
      (
        inner_mg.user_queue_latency.snapshot(),
        inner_mg.system_queue_latency.snapshot(),
      )
    };
    MailboxQueueLatencyMetrics {
      user: user_snapshot,
      system: system_snapshot,
    }
  }

  #[cfg(test)]
  pub(crate) fn test_record_queue_latency(&self, queue: MailboxQueueKind, duration: Duration) {
    let tracker = {
      let inner_mg = self.inner.lock();
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.record_for_tests(duration);
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
    self.scheduler_status.store(false, Ordering::SeqCst);
  }

  async fn compare_exchange_scheduler_status(&self, current: bool, new: bool) -> Result<bool, bool> {
    self
      .scheduler_status
      .compare_exchange(current, new, Ordering::SeqCst, Ordering::SeqCst)
  }

  async fn record_queue_enqueue(&self, queue: MailboxQueueKind) {
    let tracker = {
      let inner_mg = self.inner.lock();
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.record_enqueue();
  }

  async fn record_queue_dequeue(&self, queue: MailboxQueueKind) -> Option<Duration> {
    let tracker = {
      let inner_mg = self.inner.lock();
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.record_dequeue()
  }

  async fn clear_queue_latency(&self, queue: MailboxQueueKind) {
    let tracker = {
      let inner_mg = self.inner.lock();
      match queue {
        MailboxQueueKind::User => inner_mg.user_queue_latency.clone(),
        MailboxQueueKind::System => inner_mg.system_queue_latency.clone(),
      }
    };
    tracker.clear();
  }

  fn set_suspended(&self, suspended: bool) {
    self.suspension.set(suspended);
  }

  fn is_suspended(&self) -> bool {
    self.suspension.is_suspended()
  }

  async fn increment_system_messages_count(&self) {
    self.system_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  async fn decrement_system_messages_count(&self) {
    self.system_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn increment_user_messages_count(&self) {
    self.user_messages_count.fetch_add(1, Ordering::SeqCst);
  }

  async fn decrement_user_messages_count(&self) {
    self.user_messages_count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn get_middlewares(&self) -> Vec<MailboxMiddlewareHandle> {
    let inner_mg = self.inner.lock();
    inner_mg.middlewares.clone()
  }

  async fn poll_system_mailbox(&self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    let receiver = {
      let inner_mg = self.inner.lock();
      inner_mg.system_mailbox_reader.clone()
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
      let inner_mg = self.inner.lock();
      inner_mg.user_mailbox_reader.clone()
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
      let inner_mg = self.inner.lock();
      inner_mg.system_mailbox_writer.clone()
    };
    sender.offer(element).await
  }

  async fn offer_user_mailbox(&self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    let sender = {
      let inner_mg = self.inner.lock();
      inner_mg.user_mailbox_writer.clone()
    };
    sender.offer(element).await
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
    match self.poll_system_mailbox().await {
      Ok(Some(msg)) => {
        if let Some(latency) = self.record_queue_dequeue(MailboxQueueKind::System).await {
          message_invoker
            .record_mailbox_queue_latency(MailboxQueueKind::System, latency)
            .await;
          message_invoker
            .record_mailbox_queue_latency_snapshot(self.queue_latency_metrics())
            .await;
        }
        self.decrement_system_messages_count().await;
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
        let mut middlewares = self.get_middlewares().await;
        for middleware in &mut middlewares {
          middleware.message_received(&msg).await;
        }
        Ok(true)
      }
      Ok(None) | Err(_) => Ok(false),
    }
  }

  async fn try_handle_user_message(
    &self,
    message_invoker: &mut MessageInvokerHandle,
  ) -> Result<bool, QueueError<MessageHandle>> {
    match self.poll_user_mailbox().await {
      Ok(Some(message)) => {
        if let Some(latency) = self.record_queue_dequeue(MailboxQueueKind::User).await {
          message_invoker
            .record_mailbox_queue_latency(MailboxQueueKind::User, latency)
            .await;
          message_invoker
            .record_mailbox_queue_latency_snapshot(self.queue_latency_metrics())
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
        Ok(true)
      }
      Ok(None) | Err(_) => Ok(false),
    }
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
        tokio::task::yield_now().await;
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

      self.initialize_scheduler_status().await;
      let system_messages_count = self.get_system_messages_count().await;
      let user_messages_count = self.get_user_messages_count().await;

      if (system_messages_count > 0 || (!self.is_suspended() && user_messages_count > 0))
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
