use crate::actor::MetricsProvider;
use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter, MeterProvider};
use opentelemetry::KeyValue;
use parking_lot::RwLock;
use std::sync::Arc;

pub const LIB_NAME: &str = "protoactor";

#[derive(Debug)]
struct ActorMetricsInner {
  meter: Meter,
  actor_failure_count: Counter<u64>,
  actor_mailbox_length: Counter<u64>,
  actor_message_receive_histogram: Histogram<f64>,
  mailbox_queue_dwell_histogram: Histogram<f64>,
  mailbox_queue_dwell_percentile_gauge: Gauge<f64>,
  mailbox_queue_length_gauge: Gauge<f64>,
  mailbox_queue_dwell_bucket_total_gauge: Gauge<f64>,
  mailbox_suspension_state_gauge: Gauge<f64>,
  mailbox_suspension_resume_counter: Counter<u64>,
  mailbox_suspension_duration_histogram: Histogram<f64>,
  actor_restarted_count: Counter<u64>,
  actor_spawn_count: Counter<u64>,
  actor_stopped_count: Counter<u64>,
  dead_letter_count: Counter<u64>,
  futures_started_count: Counter<u64>,
  futures_completed_count: Counter<u64>,
  futures_timed_out_count: Counter<u64>,
  thread_pool_latency: Histogram<f64>,
  mailbox_length_histogram: Histogram<f64>,
  message_size_histogram: Histogram<f64>,
  remote_delivery_success_count: Counter<u64>,
  remote_delivery_failure_count: Counter<u64>,
  remote_receive_success_count: Counter<u64>,
  remote_receive_failure_count: Counter<u64>,
  remote_watch_event_count: Counter<u64>,
  remote_watchers_gauge: Gauge<f64>,
}

#[derive(Debug, Clone)]
pub struct ActorMetrics {
  inner: Arc<RwLock<ActorMetricsInner>>,
}

impl ActorMetrics {
  pub fn new(meter_provider: Arc<MetricsProvider>) -> Self {
    let meter = meter_provider.meter(LIB_NAME);

    ActorMetrics {
      inner: Arc::new(RwLock::new(ActorMetricsInner {
        meter: meter.clone(),
        actor_failure_count: meter
          .u64_counter("nexus_actor_actor_failure_count")
          .with_description("Number of actor failures")
          .with_unit("1")
          .build(),
        actor_mailbox_length: meter
          .u64_counter("nexus_actor_actor_mailbox_length")
          .with_description("Actor mailbox length")
          .with_unit("1")
          .build(),
        actor_message_receive_histogram: meter
          .f64_histogram("nexus_actor_actor_message_receive_duration_seconds")
          .with_description("Actor's messages received duration in seconds")
          .with_unit("s")
          .build(),
        mailbox_queue_dwell_histogram: meter
          .f64_histogram("nexus_actor_mailbox_queue_dwell_duration_seconds")
          .with_description("Time spent by messages waiting in mailbox queues before dispatch")
          .with_unit("s")
          .build(),
        mailbox_queue_dwell_percentile_gauge: meter
          .f64_gauge("nexus_actor_mailbox_queue_dwell_percentile_seconds")
          .with_description("Mailbox queue dwell duration percentiles")
          .with_unit("s")
          .build(),
        mailbox_queue_length_gauge: meter
          .f64_gauge("nexus_actor_mailbox_queue_length")
          .with_description("Current mailbox queue length")
          .with_unit("1")
          .build(),
        mailbox_queue_dwell_bucket_total_gauge: meter
          .f64_gauge("nexus_actor_mailbox_queue_dwell_bucket_total")
          .with_description("Cumulative mailbox queue dwell histogram bucket totals")
          .with_unit("1")
          .build(),
        mailbox_suspension_state_gauge: meter
          .f64_gauge("nexus_actor_mailbox_suspension_state")
          .with_description("Mailbox suspension state (1 = suspended, 0 = active)")
          .with_unit("1")
          .build(),
        mailbox_suspension_resume_counter: meter
          .u64_counter("nexus_actor_mailbox_suspension_resume_count")
          .with_description("Number of mailbox resume events")
          .with_unit("1")
          .build(),
        mailbox_suspension_duration_histogram: meter
          .f64_histogram("nexus_actor_mailbox_suspension_duration_seconds")
          .with_description("Duration of mailbox suspension intervals in seconds")
          .with_unit("s")
          .build(),
        actor_restarted_count: meter
          .u64_counter("nexus_actor_actor_restarted_count")
          .with_description("Number of actors restarts")
          .with_unit("1")
          .build(),
        actor_spawn_count: meter
          .u64_counter("nexus_actor_actor_spawn_count")
          .with_description("Number of actors spawn")
          .with_unit("1")
          .build(),
        actor_stopped_count: meter
          .u64_counter("nexus_actor_actor_stopped_count")
          .with_description("Number of actors stopped")
          .with_unit("1")
          .build(),
        dead_letter_count: meter
          .u64_counter("nexus_actor_deadletter_count")
          .with_description("Number of deadletters")
          .with_unit("1")
          .build(),
        futures_started_count: meter
          .u64_counter("nexus_actor_futures_started_count")
          .with_description("Number of futures started")
          .with_unit("1")
          .build(),
        futures_completed_count: meter
          .u64_counter("nexus_actor_futures_completed_count")
          .with_description("Number of futures completed")
          .with_unit("1")
          .build(),
        futures_timed_out_count: meter
          .u64_counter("nexus_actor_futures_timed_out_count")
          .with_description("Number of futures timed out")
          .with_unit("1")
          .build(),
        thread_pool_latency: meter
          .f64_histogram("nexus_actor_thread_pool_latency_duration_seconds")
          .with_description("History of latency in seconds")
          .with_unit("s")
          .build(),
        mailbox_length_histogram: meter
          .f64_histogram("nexus_actor_mailbox_length")
          .with_description("Mailbox length samples")
          .with_unit("1")
          .build(),
        message_size_histogram: meter
          .f64_histogram("nexus_actor_message_size_bytes")
          .with_description("Serialized message size in bytes")
          .with_unit("By")
          .build(),
        remote_delivery_success_count: meter
          .u64_counter("nexus_actor_remote_delivery_success_count")
          .with_description("Number of remote delivery successes")
          .with_unit("1")
          .build(),
        remote_delivery_failure_count: meter
          .u64_counter("nexus_actor_remote_delivery_failure_count")
          .with_description("Number of remote delivery failures")
          .with_unit("1")
          .build(),
        remote_receive_success_count: meter
          .u64_counter("nexus_actor_remote_receive_success_count")
          .with_description("Number of remote receive successes")
          .with_unit("1")
          .build(),
        remote_receive_failure_count: meter
          .u64_counter("nexus_actor_remote_receive_failure_count")
          .with_description("Number of remote receive failures")
          .with_unit("1")
          .build(),
        remote_watch_event_count: meter
          .u64_counter("nexus_actor_remote_watch_event_count")
          .with_description("Number of remote watch state change events")
          .with_unit("1")
          .build(),
        remote_watchers_gauge: meter
          .f64_gauge("nexus_actor_remote_watchers")
          .with_description("Current number of watched endpoints per watcher")
          .with_unit("1")
          .build(),
        // mailbox_length,
      })),
    }
  }

  pub fn increment_actor_failure_count(&self) {
    self.increment_actor_failure_count_with_opts(&[]);
  }

  pub fn increment_actor_failure_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.actor_failure_count.add(1, attributes);
  }

  pub fn increment_actor_mailbox_length(&self) {
    self.increment_actor_mailbox_length_with_opts(&[]);
  }

  pub fn increment_actor_mailbox_length_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.actor_mailbox_length.add(1, attributes);
  }

  pub fn record_mailbox_length(&self, length: u64) {
    self.record_mailbox_length_with_opts(length, &[]);
  }

  pub fn record_mailbox_length_with_opts(&self, length: u64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.mailbox_length_histogram.record(length as f64, attributes);
  }

  pub fn record_actor_message_receive_duration(&self, duration: f64) {
    self.record_actor_message_receive_duration_with_opts(duration, &[]);
  }

  pub fn record_actor_message_receive_duration_with_opts(&self, duration: f64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.actor_message_receive_histogram.record(duration, attributes);
  }

  pub fn record_mailbox_queue_dwell_duration(&self, duration: f64) {
    self.record_mailbox_queue_dwell_duration_with_opts(duration, &[]);
  }

  pub fn record_mailbox_queue_dwell_duration_with_opts(&self, duration: f64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.mailbox_queue_dwell_histogram.record(duration, attributes);
  }

  pub fn record_mailbox_queue_dwell_percentile(&self, value: f64) {
    self.record_mailbox_queue_dwell_percentile_with_opts(value, &[]);
  }

  pub fn record_mailbox_queue_dwell_percentile_with_opts(&self, value: f64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.mailbox_queue_dwell_percentile_gauge.record(value, attributes);
  }

  pub fn record_mailbox_queue_length(&self, length: u64) {
    self.record_mailbox_queue_length_with_opts(length, &[]);
  }

  pub fn record_mailbox_queue_length_with_opts(&self, length: u64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.mailbox_queue_length_gauge.record(length as f64, attributes);
  }

  pub fn record_mailbox_queue_dwell_bucket_total(&self, total: f64) {
    self.record_mailbox_queue_dwell_bucket_total_with_opts(total, &[]);
  }

  pub fn record_mailbox_queue_dwell_bucket_total_with_opts(&self, total: f64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg
      .mailbox_queue_dwell_bucket_total_gauge
      .record(total, attributes);
  }

  pub fn increment_mailbox_suspension_resume(&self, count: u64) {
    self.increment_mailbox_suspension_resume_with_opts(count, &[]);
  }

  pub fn increment_mailbox_suspension_resume_with_opts(&self, count: u64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.mailbox_suspension_resume_counter.add(count, attributes);
  }

  pub fn record_mailbox_suspension_duration(&self, duration: f64) {
    self.record_mailbox_suspension_duration_with_opts(duration, &[]);
  }

  pub fn record_mailbox_suspension_duration_with_opts(&self, duration: f64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg
      .mailbox_suspension_duration_histogram
      .record(duration, attributes);
  }

  pub fn record_mailbox_suspension_state(&self, suspended: bool) {
    self.record_mailbox_suspension_state_with_opts(suspended, &[]);
  }

  pub fn record_mailbox_suspension_state_with_opts(&self, suspended: bool, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    let value = if suspended { 1.0 } else { 0.0 };
    inner_mg.mailbox_suspension_state_gauge.record(value, attributes);
  }

  pub fn record_message_size(&self, size: u64) {
    self.record_message_size_with_opts(size, &[]);
  }

  pub fn record_message_size_with_opts(&self, size: u64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.message_size_histogram.record(size as f64, attributes);
  }

  pub fn increment_actor_restarted_count(&self) {
    self.increment_actor_restarted_count_with_opts(&[]);
  }

  pub fn increment_actor_restarted_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.actor_restarted_count.add(1, attributes);
  }

  pub fn increment_actor_spawn_count(&self) {
    self.increment_actor_spawn_count_with_opts(&[]);
  }

  pub fn increment_actor_spawn_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.actor_spawn_count.add(1, attributes);
  }

  pub fn increment_actor_stopped_count(&self) {
    self.increment_actor_stopped_count_with_opts(&[]);
  }

  pub fn increment_actor_stopped_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.actor_stopped_count.add(1, attributes);
  }

  pub fn increment_dead_letter_count(&self) {
    self.increment_dead_letter_count_with_opts(&[]);
  }

  pub fn increment_dead_letter_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.dead_letter_count.add(1, attributes);
  }

  pub fn increment_futures_started_count(&self) {
    self.increment_futures_started_count_with_opts(&[])
  }

  pub fn increment_futures_started_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.futures_started_count.add(1, attributes);
  }

  pub fn increment_futures_completed_count(&self) {
    self.increment_futures_completed_count_with_opts(&[]);
  }

  pub fn increment_futures_completed_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.futures_completed_count.add(1, attributes);
  }

  pub fn increment_futures_timed_out_count(&self) {
    self.increment_futures_timed_out_count_with_opts(&[]);
  }

  pub fn increment_futures_timed_out_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.futures_timed_out_count.add(1, attributes);
  }

  pub fn increment_remote_delivery_success_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.remote_delivery_success_count.add(1, attributes);
  }

  pub fn increment_remote_delivery_success(&self) {
    self.increment_remote_delivery_success_with_opts(&[]);
  }

  pub fn increment_remote_delivery_failure_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.remote_delivery_failure_count.add(1, attributes);
  }

  pub fn increment_remote_delivery_failure(&self) {
    self.increment_remote_delivery_failure_with_opts(&[]);
  }

  pub fn increment_remote_receive_success_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.remote_receive_success_count.add(1, attributes);
  }

  pub fn increment_remote_receive_success(&self) {
    self.increment_remote_receive_success_with_opts(&[]);
  }

  pub fn increment_remote_receive_failure_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.remote_receive_failure_count.add(1, attributes);
  }

  pub fn increment_remote_receive_failure(&self) {
    self.increment_remote_receive_failure_with_opts(&[]);
  }

  pub fn increment_remote_watch_event(&self) {
    self.increment_remote_watch_event_with_opts(&[]);
  }

  pub fn increment_remote_watch_event_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.remote_watch_event_count.add(1, attributes);
  }

  pub fn record_remote_watchers(&self, watchers: u32) {
    self.record_remote_watchers_with_opts(watchers, &[]);
  }

  pub fn record_remote_watchers_with_opts(&self, watchers: u32, attributes: &[KeyValue]) {
    let inner_mg = self.inner.read();
    inner_mg.remote_watchers_gauge.record(watchers as f64, attributes);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use opentelemetry_sdk::metrics::SdkMeterProvider;

  #[test]
  fn test_actor_metrics() {
    let meter_provider = MetricsProvider::Sdk(SdkMeterProvider::default());
    let metrics = ActorMetrics::new(Arc::new(meter_provider));

    metrics.increment_actor_failure_count();
    metrics.increment_actor_mailbox_length();
    metrics.increment_actor_restarted_count();
    metrics.increment_actor_spawn_count();
    metrics.increment_actor_stopped_count();
    metrics.increment_dead_letter_count();
    metrics.increment_futures_started_count();
    metrics.increment_futures_completed_count();
    metrics.increment_futures_timed_out_count();
  }
}
