use crate::actor::actor_system::ActorSystem;
use crate::extensions::{next_extension_id, Extension, ExtensionId};
use crate::metrics::{ActorMetrics, MetricsError, ProtoMetrics};
use arc_swap::ArcSwapOption;
use once_cell::sync::Lazy;
use opentelemetry::KeyValue;
use std::any::Any;
use std::sync::Arc;

pub static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(next_extension_id);

#[derive(Debug, Clone)]
pub struct Metrics {
  runtime: Arc<ArcSwapOption<MetricsRuntime>>,
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
    let metrics_provider = system.get_config().metrics_provider.clone();
    let runtime = match metrics_provider {
      Some(provider) => {
        let address = system.get_address().await;
        let proto_metrics = ProtoMetrics::new(provider.clone())?;
        Some(Arc::new(MetricsRuntime::new(address, proto_metrics)))
      }
      None => None,
    };

    runtime_slot.store(runtime.clone());

    Ok(Metrics { runtime: runtime_slot })
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
mod tests {
  use super::*;
  use crate::actor::config::MetricsProvider;
  use crate::metrics::ProtoMetrics;
  use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
  use std::sync::Arc;

  #[test]
  fn test_mailbox_queue_dwell_percentile_is_exported() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let metrics_provider = Arc::new(MetricsProvider::Sdk(provider.clone()));
    let proto_metrics = ProtoMetrics::new(metrics_provider).expect("metrics init");
    let runtime = MetricsRuntime::new("test-system".to_string(), proto_metrics);
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
                if is_user && is_p50
                {
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
}
