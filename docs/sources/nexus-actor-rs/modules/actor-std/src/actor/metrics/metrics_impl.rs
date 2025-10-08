use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ActorProcess;
use crate::actor::dispatch::{MailboxQueueKind, MailboxQueueLatencyMetrics, MailboxSyncHandle};
use crate::actor::process::process_registry::ProcessRegistry;
use crate::actor::process::Process;
use crate::extensions::{next_extension_id, Extension, ExtensionId};
use crate::metrics::{ActorMetrics, MetricsError, ProtoMetrics};
use arc_swap::ArcSwapOption;
use nexus_actor_core_rs::runtime::{CoreRuntime, CoreScheduledHandleRef, CoreScheduledTask};
use once_cell::sync::Lazy;
use opentelemetry::KeyValue;
use parking_lot::Mutex;
use std::any::Any;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(next_extension_id);

#[derive(Debug, Clone)]
pub struct Metrics {
  runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  mailbox_collector: Option<Arc<MailboxMetricsCollector>>,
}

impl Extension for Metrics {
  fn extension_id(&self) -> ExtensionId {
    *EXTENSION_ID
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

impl Metrics {
  pub async fn new(
    system: ActorSystem,
    runtime_slot: Arc<ArcSwapOption<MetricsRuntime>>,
  ) -> Result<Self, MetricsError> {
    let config = system.get_config();
    let metrics_provider = config.metrics_provider.clone();
    let poll_interval = config.mailbox_metrics_poll_interval;
    let factory = match metrics_provider {
      Some(provider) => {
        let address = system.get_address().await;
        let proto_metrics = ProtoMetrics::new(provider.clone())?;
        Some(Arc::new(MetricsRuntime::new(address, proto_metrics)))
      }
      None => None,
    };

    runtime_slot.store(runtime.clone());

    let core_runtime = system.core_runtime();
    let mailbox_collector = runtime
      .as_ref()
      .map(|runtime| MailboxMetricsCollector::spawn(&system, runtime.clone(), poll_interval, core_runtime.clone()));

    Ok(Metrics {
      runtime: runtime_slot,
      mailbox_collector,
    })
  }

  pub fn runtime(&self) -> Option<Arc<MetricsRuntime>> {
    self.runtime.load_full()
  }

  pub fn is_enabled(&self) -> bool {
    self.runtime().is_some()
  }

  pub fn foreach<F>(&self, f: F)
  where
    F: FnOnce(&ActorMetrics, &MetricsRuntime), {
    if let Some(runtime) = self.runtime() {
      let actor_metrics = runtime.actor_metrics();
      f(&actor_metrics, &runtime);
    }
  }
}

#[derive(Debug, Clone)]
pub struct MetricsRuntime {
  proto_metrics: ProtoMetrics,
  address: Arc<str>,
}

impl MetricsRuntime {
  pub fn new(address: String, proto_metrics: ProtoMetrics) -> Self {
    Self {
      proto_metrics,
      address: Arc::from(address.into_boxed_str()),
    }
  }

  pub fn actor_metrics(&self) -> ActorMetrics {
    self
      .proto_metrics
      .get(ProtoMetrics::INTERNAL_ACTOR_METRICS)
      .expect("internal actor metrics must exist")
  }

  pub fn sink_for_actor<'a>(&self, actor_type: Option<&'a str>) -> MetricsSink {
    let common_labels = build_common_labels(&self.address, actor_type);
    MetricsSink::new(self.actor_metrics(), common_labels)
  }

  pub fn sink_without_actor(&self) -> MetricsSink {
    self.sink_for_actor(None)
  }

  pub fn address(&self) -> &str {
    &self.address
  }
}

struct MailboxMetricsCollector {
  handle: CoreScheduledHandleRef,
  stop: Arc<AtomicBool>,
}

impl MailboxMetricsCollector {
  fn spawn(
    system: &ActorSystem,
    runtime: Arc<MetricsRuntime>,
    poll_interval: Duration,
    core_runtime: CoreRuntime,
  ) -> Arc<Self> {
    let weak_system = system.downgrade();
    let interval = if poll_interval.is_zero() {
      Duration::from_millis(1)
    } else {
      poll_interval
    };
    let scheduler = core_runtime.scheduler();
    let stop = Arc::new(AtomicBool::new(false));
    let handle_slot: Arc<Mutex<Option<CoreScheduledHandleRef>>> = Arc::new(Mutex::new(None));
    let task_runtime = runtime.clone();

    let poll_task: CoreScheduledTask = {
      let weak_system = weak_system.clone();
      let stop_flag = stop.clone();
      let handle_slot = handle_slot.clone();
      let task_runtime = task_runtime.clone();
      Arc::new(move || {
        let weak_system = weak_system.clone();
        let stop_flag = stop_flag.clone();
        let handle_slot = handle_slot.clone();
        let task_runtime = task_runtime.clone();
        Box::pin(async move {
          if stop_flag.load(Ordering::Relaxed) {
            if let Some(handle) = handle_slot.lock().clone() {
              handle.cancel();
            }
            return;
          }

          let Some(system) = weak_system.upgrade() else {
            stop_flag.store(true, Ordering::Relaxed);
            if let Some(handle) = handle_slot.lock().clone() {
              handle.cancel();
            }
            return;
          };

          collect_mailbox_metrics(task_runtime.clone(), system).await;
        })
      })
    };

    let handle = scheduler.schedule_repeated(Duration::ZERO, interval, poll_task);
    *handle_slot.lock() = Some(handle.clone());

    Arc::new(Self { handle, stop })
  }
}

impl fmt::Debug for MailboxMetricsCollector {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxMetricsCollector")
      .field("handle", &format_args!("{:p}", Arc::as_ptr(&self.handle)))
      .field("stopped", &self.stop.load(Ordering::Relaxed))
      .finish()
  }
}

impl Drop for MailboxMetricsCollector {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Relaxed);
    self.handle.cancel();
  }
}

async fn collect_mailbox_metrics(runtime: Arc<MetricsRuntime>, system: ActorSystem) {
  let registry: ProcessRegistry = system.get_process_registry().await;
  let pids = registry.list_local_pids().await;
  if pids.is_empty() {
    return;
  }

  let sink = runtime.sink_without_actor();
  let base_labels: Vec<KeyValue> = sink.common_labels().to_vec();
  let actor_metrics = sink.actor_metrics().clone();
  drop(sink);

  for pid in pids {
    let pid_id = pid.id.clone();
    let Some(process) = registry.find_local_process_handle(&pid_id).await else {
      continue;
    };
    let Some(actor_process) = process.as_any().downcast_ref::<ActorProcess>() else {
      continue;
    };
    let mailbox = actor_process.mailbox_handle();
    let Some(sync) = mailbox.sync_handle() else {
      continue;
    };
    let pid_label = KeyValue::new("pid", pid_id.clone());
    record_queue_lengths(&actor_metrics, &base_labels, &pid_label, &sync);
    let latency_metrics = sync.queue_latency_metrics();
    record_queue_latency_percentiles(&actor_metrics, &base_labels, &pid_label, &latency_metrics);
    record_mailbox_suspension_state(&actor_metrics, &base_labels, &pid_label, sync.is_suspended());
  }
}

fn record_queue_lengths(
  actor_metrics: &ActorMetrics,
  base_labels: &[KeyValue],
  pid_label: &KeyValue,
  sync: &MailboxSyncHandle,
) {
  let core_handles = sync.core_queue_handles();
  for queue in [MailboxQueueKind::User, MailboxQueueKind::System] {
    let length = if let Some((ref user_core, ref system_core)) = core_handles {
      match queue {
        MailboxQueueKind::User => user_core.len() as u64,
        MailboxQueueKind::System => system_core.len() as u64,
      }
    } else {
      match queue {
        MailboxQueueKind::User => sync.user_messages_count().max(0) as u64,
        MailboxQueueKind::System => sync.system_messages_count().max(0) as u64,
      }
    };

    let mut labels = Vec::with_capacity(base_labels.len() + 2);
    labels.extend_from_slice(base_labels);
    labels.push(pid_label.clone());
    labels.push(KeyValue::new("queue_kind", queue.as_str().to_string()));
    actor_metrics.record_mailbox_queue_length_with_opts(length, &labels);
  }
}

fn record_queue_latency_percentiles(
  actor_metrics: &ActorMetrics,
  base_labels: &[KeyValue],
  pid_label: &KeyValue,
  metrics: &MailboxQueueLatencyMetrics,
) {
  const PERCENTILES: [(&str, f64); 3] = [("p50", 50.0), ("p95", 95.0), ("p99", 99.0)];
  for queue in [MailboxQueueKind::User, MailboxQueueKind::System] {
    for &(label, percentile) in PERCENTILES.iter() {
      if let Some(duration) = metrics.percentile(queue, percentile) {
        let mut labels = Vec::with_capacity(base_labels.len() + 3);
        labels.extend_from_slice(base_labels);
        labels.push(pid_label.clone());
        labels.push(KeyValue::new("queue_kind", queue.as_str().to_string()));
        labels.push(KeyValue::new("percentile", label.to_string()));
        actor_metrics.record_mailbox_queue_dwell_percentile_with_opts(duration.as_secs_f64(), &labels);
      }
    }
  }
}

fn record_mailbox_suspension_state(
  actor_metrics: &ActorMetrics,
  base_labels: &[KeyValue],
  pid_label: &KeyValue,
  suspended: bool,
) {
  let mut labels = Vec::with_capacity(base_labels.len() + 1);
  labels.extend_from_slice(base_labels);
  labels.push(pid_label.clone());
  actor_metrics.record_mailbox_suspension_state_with_opts(suspended, &labels);
}

#[derive(Debug, Clone)]
pub struct MetricsSink {
  actor_metrics: ActorMetrics,
  common_labels: Arc<[KeyValue]>,
}

impl MetricsSink {
  fn new(actor_metrics: ActorMetrics, common_labels: Arc<[KeyValue]>) -> Self {
    Self {
      actor_metrics,
      common_labels,
    }
  }

  fn merge_labels(&self, additional: &[KeyValue]) -> Vec<KeyValue> {
    if additional.is_empty() {
      return self.common_labels.to_vec();
    }
    let mut merged = Vec::with_capacity(self.common_labels.len() + additional.len());
    merged.extend_from_slice(&self.common_labels);
    merged.extend_from_slice(additional);
    merged
  }

  pub fn increment_actor_spawn(&self) {
    self
      .actor_metrics
      .increment_actor_spawn_count_with_opts(&self.common_labels);
  }

  pub fn increment_actor_restarted(&self) {
    self
      .actor_metrics
      .increment_actor_restarted_count_with_opts(&self.common_labels);
  }

  pub fn increment_actor_stopped(&self) {
    self
      .actor_metrics
      .increment_actor_stopped_count_with_opts(&self.common_labels);
  }

  pub fn increment_actor_failure(&self) {
    self
      .actor_metrics
      .increment_actor_failure_count_with_opts(&self.common_labels);
  }

  pub fn increment_actor_failure_with_additional_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_actor_failure_count_with_opts(&merged);
  }

  pub fn record_message_receive_duration(&self, duration: f64) {
    self
      .actor_metrics
      .record_actor_message_receive_duration_with_opts(duration, &self.common_labels);
  }

  pub fn record_message_receive_duration_with_type(&self, duration: f64, message_type: Option<&str>) {
    if let Some(mt) = message_type {
      let merged = self.merge_labels(&[KeyValue::new("message_type", mt.replace('*', ""))]);
      self
        .actor_metrics
        .record_actor_message_receive_duration_with_opts(duration, &merged);
    } else {
      self.record_message_receive_duration(duration);
    }
  }

  pub fn record_mailbox_length(&self, length: u64) {
    self
      .actor_metrics
      .record_mailbox_length_with_opts(length, &self.common_labels);
  }

  pub fn record_mailbox_length_with_labels(&self, length: u64, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.record_mailbox_length_with_opts(length, &merged);
  }

  pub fn record_message_size(&self, size: u64) {
    self
      .actor_metrics
      .record_message_size_with_opts(size, &self.common_labels);
  }

  pub fn record_message_size_with_labels(&self, size: u64, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.record_message_size_with_opts(size, &merged);
  }

  pub fn record_mailbox_queue_dwell_duration(&self, duration: f64, queue_kind: &str) {
    let merged = self.merge_labels(&[KeyValue::new("queue_kind", queue_kind.to_string())]);
    self
      .actor_metrics
      .record_mailbox_queue_dwell_duration_with_opts(duration, &merged);
  }

  pub fn record_mailbox_queue_dwell_percentile(&self, percentile_label: &str, duration: f64, queue_kind: &str) {
    let merged = self.merge_labels(&[
      KeyValue::new("queue_kind", queue_kind.to_string()),
      KeyValue::new("percentile", percentile_label.to_string()),
    ]);
    self
      .actor_metrics
      .record_mailbox_queue_dwell_percentile_with_opts(duration, &merged);
  }

  pub fn record_mailbox_queue_dwell_bucket_total(&self, bucket_label: &str, queue_kind: &str, total: u64) {
    let merged = self.merge_labels(&[
      KeyValue::new("queue_kind", queue_kind.to_string()),
      KeyValue::new("bucket", bucket_label.to_string()),
    ]);
    self
      .actor_metrics
      .record_mailbox_queue_dwell_bucket_total_with_opts(total as f64, &merged);
  }

  pub fn record_mailbox_queue_length(&self, length: u64, queue_kind: &str) {
    let merged = self.merge_labels(&[KeyValue::new("queue_kind", queue_kind.to_string())]);
    self
      .actor_metrics
      .record_mailbox_queue_length_with_opts(length, &merged);
  }

  pub fn increment_mailbox_suspension_resume(&self, count: u64) {
    if count == 0 {
      return;
    }
    self
      .actor_metrics
      .increment_mailbox_suspension_resume_with_opts(count, &self.common_labels);
  }

  pub fn record_mailbox_suspension_duration(&self, duration: f64) {
    if duration <= 0.0 {
      return;
    }
    self
      .actor_metrics
      .record_mailbox_suspension_duration_with_opts(duration, &self.common_labels);
  }

  pub fn record_mailbox_suspension_state(&self, suspended: bool) {
    self
      .actor_metrics
      .record_mailbox_suspension_state_with_opts(suspended, &self.common_labels);
  }

  pub fn increment_dead_letter(&self) {
    self
      .actor_metrics
      .increment_dead_letter_count_with_opts(&self.common_labels);
  }

  pub fn increment_dead_letter_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_dead_letter_count_with_opts(&merged);
  }

  pub fn increment_future_started(&self) {
    self
      .actor_metrics
      .increment_futures_started_count_with_opts(&self.common_labels);
  }

  pub fn increment_future_started_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_futures_started_count_with_opts(&merged);
  }

  pub fn increment_future_completed(&self) {
    self
      .actor_metrics
      .increment_futures_completed_count_with_opts(&self.common_labels);
  }

  pub fn increment_future_completed_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_futures_completed_count_with_opts(&merged);
  }

  pub fn increment_future_timed_out(&self) {
    self
      .actor_metrics
      .increment_futures_timed_out_count_with_opts(&self.common_labels);
  }

  pub fn increment_future_timed_out_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_futures_timed_out_count_with_opts(&merged);
  }

  pub fn increment_remote_delivery_success_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_remote_delivery_success_with_opts(&merged);
  }

  pub fn increment_remote_delivery_success(&self) {
    self.increment_remote_delivery_success_with_labels(&[]);
  }

  pub fn increment_remote_delivery_failure_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_remote_delivery_failure_with_opts(&merged);
  }

  pub fn increment_remote_delivery_failure(&self) {
    self.increment_remote_delivery_failure_with_labels(&[]);
  }

  pub fn increment_remote_receive_success_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_remote_receive_success_with_opts(&merged);
  }

  pub fn increment_remote_receive_success(&self) {
    self.increment_remote_receive_success_with_labels(&[]);
  }

  pub fn increment_remote_receive_failure_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_remote_receive_failure_with_opts(&merged);
  }

  pub fn increment_remote_receive_failure(&self) {
    self.increment_remote_receive_failure_with_labels(&[]);
  }

  pub fn increment_remote_watch_event_with_labels(&self, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.increment_remote_watch_event_with_opts(&merged);
  }

  pub fn increment_remote_watch_event(&self) {
    self.increment_remote_watch_event_with_labels(&[]);
  }

  pub fn record_remote_watchers_with_labels(&self, watchers: u32, additional: &[KeyValue]) {
    let merged = self.merge_labels(additional);
    self.actor_metrics.record_remote_watchers_with_opts(watchers, &merged);
  }

  pub fn record_remote_watchers(&self, watchers: u32) {
    self.record_remote_watchers_with_labels(watchers, &[]);
  }

  pub fn actor_metrics(&self) -> &ActorMetrics {
    &self.actor_metrics
  }

  pub fn common_labels(&self) -> &[KeyValue] {
    &self.common_labels
  }
}

fn build_common_labels(address: &str, actor_type: Option<&str>) -> Arc<[KeyValue]> {
  let mut labels: Vec<KeyValue> = Vec::with_capacity(2);
  labels.push(KeyValue::new("address", address.to_string()));
  if let Some(actor_type) = actor_type {
    labels.push(KeyValue::new("actor_type", actor_type.replace('*', "")));
  }
  Arc::from(labels.into_boxed_slice())
}

#[cfg(test)]
impl MailboxMetricsCollector {
  fn handle_ref(&self) -> CoreScheduledHandleRef {
    self.handle.clone()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::config::MetricsProvider;
  use crate::actor::context::SpawnerPart;
  use crate::actor::core::Props;
  use crate::actor::ConfigOption;
  use crate::metrics::ProtoMetrics;
  use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::time::sleep;

  #[tokio::test(flavor = "current_thread")]
  async fn mailbox_metrics_collector_cancels_handle_on_drop() {
    let system = ActorSystem::new_config_options([
      ConfigOption::SetMetricsProvider(Arc::new(MetricsProvider::Global)),
      ConfigOption::with_mailbox_metrics_poll_interval(Duration::from_millis(5)),
    ])
    .await
    .expect("actor system");

    let address = system.get_address().await;
    let provider = Arc::new(MetricsProvider::Global);
    let proto_metrics = ProtoMetrics::new(provider).expect("proto metrics");
    let metrics_runtime = Arc::new(MetricsRuntime::new(address, proto_metrics));

    let collector = MailboxMetricsCollector::spawn(
      &system,
      metrics_runtime,
      Duration::from_millis(5),
      system.core_runtime(),
    );

    let handle = collector.handle_ref();
    assert!(!handle.is_cancelled());

    drop(collector);

    sleep(Duration::from_millis(20)).await;
    assert!(handle.is_cancelled());
  }

  #[test]
  fn test_mailbox_queue_dwell_percentile_is_exported() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let metrics_provider = Arc::new(MetricsProvider::Sdk(provider.clone()));
    let proto_metrics = ProtoMetrics::new(metrics_provider).expect("metrics init");
    let factory = MetricsRuntime::new("test-system".to_string(), proto_metrics);
    let sink = runtime.sink_without_actor();

    sink.record_mailbox_queue_dwell_percentile("p50", 0.123, "user");

    provider.force_flush().expect("force flush");
    let exported = exporter.get_finished_metrics().expect("exported metrics");

    use opentelemetry::Value;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    let mut found = false;
    for resource in exported {
      for scope in resource.scope_metrics() {
        for metric in scope.metrics() {
          if metric.name() == "nexus_actor_mailbox_queue_dwell_percentile_seconds" {
            if let AggregatedMetrics::F64(MetricData::Gauge(gauge)) = metric.data() {
              for data_point in gauge.data_points() {
                let mut queue_kind = None;
                let mut percentile = None;
                for kv in data_point.attributes() {
                  match kv.key.as_str() {
                    "queue_kind" => queue_kind = Some(&kv.value),
                    "percentile" => percentile = Some(&kv.value),
                    _ => {}
                  }
                }
                let is_user = matches!(queue_kind, Some(Value::String(ref s)) if s.as_ref() == "user");
                let is_p50 = matches!(percentile, Some(Value::String(ref s)) if s.as_ref() == "p50");
                if is_user && is_p50 {
                  assert!((data_point.value() - 0.123).abs() < f64::EPSILON);
                  found = true;
                }
              }
            }
          }
        }
      }
    }

    assert!(found, "percentile gauge should be exported");
  }

  #[test]
  fn test_mailbox_queue_dwell_bucket_total_is_exported() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let metrics_provider = Arc::new(MetricsProvider::Sdk(provider.clone()));
    let proto_metrics = ProtoMetrics::new(metrics_provider).expect("metrics init");
    let factory = MetricsRuntime::new("test-system".to_string(), proto_metrics);
    let sink = runtime.sink_without_actor();

    sink.record_mailbox_queue_dwell_bucket_total("bucket_2", "system", 42);

    provider.force_flush().expect("force flush");
    let exported = exporter.get_finished_metrics().expect("exported metrics");

    use opentelemetry::Value;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    let mut found = false;
    for resource in exported {
      for scope in resource.scope_metrics() {
        for metric in scope.metrics() {
          if metric.name() == "nexus_actor_mailbox_queue_dwell_bucket_total" {
            if let AggregatedMetrics::F64(MetricData::Gauge(gauge)) = metric.data() {
              for data_point in gauge.data_points() {
                let mut queue_kind = None;
                let mut bucket = None;
                for kv in data_point.attributes() {
                  match kv.key.as_str() {
                    "queue_kind" => queue_kind = Some(&kv.value),
                    "bucket" => bucket = Some(&kv.value),
                    _ => {}
                  }
                }
                let is_system = matches!(queue_kind, Some(Value::String(ref s)) if s.as_ref() == "system");
                let is_bucket_2 = matches!(bucket, Some(Value::String(ref s)) if s.as_ref() == "bucket_2");
                if is_system && is_bucket_2 {
                  assert!((data_point.value() - 42.0).abs() < f64::EPSILON);
                  found = true;
                }
              }
            }
          }
        }
      }
    }

    assert!(found, "bucket gauge should be exported");
  }

  #[test]
  fn test_mailbox_suspension_metrics_are_exported() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let metrics_provider = Arc::new(MetricsProvider::Sdk(provider.clone()));
    let proto_metrics = ProtoMetrics::new(metrics_provider).expect("metrics init");
    let factory = MetricsRuntime::new("test-system".to_string(), proto_metrics);
    let sink = runtime.sink_without_actor();

    sink.increment_mailbox_suspension_resume(3);
    sink.record_mailbox_suspension_duration(1.5);
    sink.record_mailbox_suspension_state(true);

    provider.force_flush().expect("force flush");
    let exported = exporter.get_finished_metrics().expect("exported metrics");

    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    let mut resume_found = false;
    let mut duration_found = false;
    let mut state_found = false;

    for resource in exported {
      for scope in resource.scope_metrics() {
        for metric in scope.metrics() {
          match metric.name() {
            "nexus_actor_mailbox_suspension_resume_count" => {
              if let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() {
                for data_point in sum.data_points() {
                  if data_point.value() == 3 {
                    resume_found = true;
                  }
                }
              }
            }
            "nexus_actor_mailbox_suspension_duration_seconds" => {
              if let AggregatedMetrics::F64(MetricData::Histogram(hist)) = metric.data() {
                for data_point in hist.data_points() {
                  if (data_point.sum() - 1.5).abs() < f64::EPSILON {
                    duration_found = true;
                  }
                }
              }
            }
            "nexus_actor_mailbox_suspension_state" => {
              if let AggregatedMetrics::F64(MetricData::Gauge(gauge)) = metric.data() {
                for data_point in gauge.data_points() {
                  if (data_point.value() - 1.0).abs() < f64::EPSILON {
                    state_found = true;
                  }
                }
              }
            }
            _ => {}
          }
        }
      }
    }

    assert!(resume_found, "resume counter should be exported");
    assert!(duration_found, "duration histogram should be exported");
    assert!(state_found, "state gauge should be exported");
  }

  #[test]
  fn test_remote_watch_metrics_are_exported() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let metrics_provider = Arc::new(MetricsProvider::Sdk(provider.clone()));
    let proto_metrics = ProtoMetrics::new(metrics_provider).expect("metrics init");
    let factory = MetricsRuntime::new("test-system".to_string(), proto_metrics);
    let sink = runtime.sink_without_actor();

    let event_labels = [
      KeyValue::new("remote.endpoint", "endpoint-1".to_string()),
      KeyValue::new("remote.watcher", "watcher-1".to_string()),
      KeyValue::new("remote.watch_action", "watch".to_string()),
    ];
    sink.increment_remote_watch_event_with_labels(&event_labels);

    let gauge_labels = [
      KeyValue::new("remote.endpoint", "endpoint-1".to_string()),
      KeyValue::new("remote.watcher", "watcher-1".to_string()),
    ];
    sink.record_remote_watchers_with_labels(5, &gauge_labels);

    provider.force_flush().expect("force flush");
    let exported = exporter.get_finished_metrics().expect("exported metrics");

    use opentelemetry::Value;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    let mut event_found = false;
    let mut gauge_found = false;

    for resource in exported {
      for scope in resource.scope_metrics() {
        for metric in scope.metrics() {
          match metric.name() {
            "nexus_actor_remote_watch_event_count" => {
              if let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() {
                for data_point in sum.data_points() {
                  if data_point.value() == 1 {
                    let mut endpoint = None;
                    let mut watcher = None;
                    let mut action = None;
                    for kv in data_point.attributes() {
                      match kv.key.as_str() {
                        "remote.endpoint" => endpoint = Some(&kv.value),
                        "remote.watcher" => watcher = Some(&kv.value),
                        "remote.watch_action" => action = Some(&kv.value),
                        _ => {}
                      }
                    }
                    let endpoint_ok = matches!(endpoint, Some(Value::String(ref s)) if s.as_ref() == "endpoint-1");
                    let watcher_ok = matches!(watcher, Some(Value::String(ref s)) if s.as_ref() == "watcher-1");
                    let action_ok = matches!(action, Some(Value::String(ref s)) if s.as_ref() == "watch");
                    if endpoint_ok && watcher_ok && action_ok {
                      event_found = true;
                    }
                  }
                }
              }
            }
            "nexus_actor_remote_watchers" => {
              if let AggregatedMetrics::F64(MetricData::Gauge(gauge)) = metric.data() {
                for data_point in gauge.data_points() {
                  let mut endpoint = None;
                  let mut watcher = None;
                  for kv in data_point.attributes() {
                    match kv.key.as_str() {
                      "remote.endpoint" => endpoint = Some(&kv.value),
                      "remote.watcher" => watcher = Some(&kv.value),
                      _ => {}
                    }
                  }
                  let endpoint_ok = matches!(endpoint, Some(Value::String(ref s)) if s.as_ref() == "endpoint-1");
                  let watcher_ok = matches!(watcher, Some(Value::String(ref s)) if s.as_ref() == "watcher-1");
                  if endpoint_ok && watcher_ok && (data_point.value() - 5.0).abs() < f64::EPSILON {
                    gauge_found = true;
                  }
                }
              }
            }
            _ => {}
          }
        }
      }
    }

    assert!(event_found, "remote watch event counter should be exported");
    assert!(gauge_found, "remote watchers gauge should be exported");
  }

  #[tokio::test]
  async fn test_mailbox_metrics_collector_emits_queue_length_with_pid() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let metrics_provider = Arc::new(MetricsProvider::Sdk(provider.clone()));

    let system = ActorSystem::new_config_options([
      ConfigOption::SetMetricsProvider(metrics_provider.clone()),
      ConfigOption::with_mailbox_metrics_poll_interval(Duration::from_millis(10)),
    ])
    .await
    .expect("actor system with metrics");

    let mut root = system.get_root_context().await;
    let props = Props::from_async_actor_receiver(|_| async { Ok(()) }).await;
    let pid = root.spawn(props).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    provider.force_flush().expect("force flush");
    let exported = exporter.get_finished_metrics().expect("metrics exported");

    use opentelemetry::Value;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};

    let pid_value = pid.id().to_string();
    let mut found = false;

    for resource in exported {
      for scope in resource.scope_metrics() {
        for metric in scope.metrics() {
          if metric.name() == "nexus_actor_mailbox_queue_length" {
            if let AggregatedMetrics::F64(MetricData::Gauge(gauge)) = metric.data() {
              for data_point in gauge.data_points() {
                let mut has_pid = false;
                let mut is_user_queue = false;
                for kv in data_point.attributes() {
                  match kv.key.as_str() {
                    "pid" => {
                      if let Value::String(ref s) = kv.value {
                        has_pid = s.as_ref() == pid_value.as_str();
                      }
                    }
                    "queue_kind" => {
                      if let Value::String(ref s) = kv.value {
                        is_user_queue = s.as_ref() == "user";
                      }
                    }
                    _ => {}
                  }
                }
                if has_pid && is_user_queue {
                  found = true;
                }
              }
            }
          }
        }
      }
    }

    assert!(found, "collector should export queue length with pid label");
  }
}
