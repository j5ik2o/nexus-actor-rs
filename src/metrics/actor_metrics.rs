use opentelemetry::metrics::MeterProvider;
use opentelemetry::metrics::{Counter, Histogram, Meter, ObservableGauge};
use opentelemetry_sdk::metrics::reader::MetricReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::sync::{Arc, RwLock};
const LIB_NAME: &str = "protoactor";

#[derive(Debug, Clone)]
pub struct ActorMetrics {
  meter: Meter,
  actor_failure_count: Counter<u64>,
  actor_mailbox_length: ObservableGauge<i64>,
  actor_message_receive_histogram: Histogram<f64>,
  actor_restarted_count: Counter<u64>,
  actor_spawn_count: Counter<u64>,
  actor_stopped_count: Counter<u64>,
  dead_letter_count: Counter<u64>,
  futures_started_count: Counter<u64>,
  futures_completed_count: Counter<u64>,
  futures_timed_out_count: Counter<u64>,
  thread_pool_latency: Histogram<f64>,
  mailbox_length: Arc<RwLock<i64>>,
}

impl ActorMetrics {
  pub fn new(reader: impl MetricReader) -> Result<Self, opentelemetry::metrics::MetricsError> {
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    let meter = provider.meter(LIB_NAME);

    let mailbox_length = Arc::new(RwLock::new(0i64));
    let mailbox_length_clone = mailbox_length.clone();

    let actor_mailbox_length = meter
      .i64_observable_gauge("nexus_actor_actor_mailbox_length")
      .with_description("Actor mailbox length")
      .with_unit("1")
      .try_init()?;

    let cloned_actor_mailbox_length = actor_mailbox_length.clone();

    meter.register_callback(&[actor_mailbox_length.as_any()], move |observer| {
      if let Ok(length) = mailbox_length_clone.read() {
        observer.observe_i64(&cloned_actor_mailbox_length, *length, &[]);
      }
    })?;

    Ok(ActorMetrics {
      meter: meter.clone(),
      actor_failure_count: meter
        .u64_counter("nexus_actor_actor_failure_count")
        .with_description("Number of actor failures")
        .with_unit("1")
        .try_init()?,
      actor_mailbox_length,
      actor_message_receive_histogram: meter
        .f64_histogram("nexus_actor_actor_message_receive_duration_seconds")
        .with_description("Actor's messages received duration in seconds")
        .with_unit("s")
        .try_init()?,
      actor_restarted_count: meter
        .u64_counter("nexus_actor_actor_restarted_count")
        .with_description("Number of actors restarts")
        .with_unit("1")
        .try_init()?,
      actor_spawn_count: meter
        .u64_counter("nexus_actor_actor_spawn_count")
        .with_description("Number of actors spawn")
        .with_unit("1")
        .try_init()?,
      actor_stopped_count: meter
        .u64_counter("nexus_actor_actor_stopped_count")
        .with_description("Number of actors stopped")
        .with_unit("1")
        .try_init()?,
      dead_letter_count: meter
        .u64_counter("nexus_actor_deadletter_count")
        .with_description("Number of deadletters")
        .with_unit("1")
        .try_init()?,
      futures_started_count: meter
        .u64_counter("nexus_actor_futures_started_count")
        .with_description("Number of futures started")
        .with_unit("1")
        .try_init()?,
      futures_completed_count: meter
        .u64_counter("nexus_actor_futures_completed_count")
        .with_description("Number of futures completed")
        .with_unit("1")
        .try_init()?,
      futures_timed_out_count: meter
        .u64_counter("nexus_actor_futures_timed_out_count")
        .with_description("Number of futures timed out")
        .with_unit("1")
        .try_init()?,
      thread_pool_latency: meter
        .f64_histogram("nexus_actor_thread_pool_latency_duration_seconds")
        .with_description("History of latency in seconds")
        .with_unit("s")
        .try_init()?,
      mailbox_length,
    })
  }

  pub fn increment_actor_failure_count(&self) {
    self.actor_failure_count.add(1, &[]);
  }

  pub fn set_actor_mailbox_length(&self, length: i64) {
    *self.mailbox_length.write().unwrap() = length;
  }

  pub fn get_actor_mailbox_length(&self) -> i64 {
    *self.mailbox_length.read().unwrap()
  }

  pub fn record_actor_message_receive_duration(&self, duration: f64) {
    self.actor_message_receive_histogram.record(duration, &[]);
  }

  pub fn increment_actor_restarted_count(&self) {
    self.actor_restarted_count.add(1, &[]);
  }

  pub fn increment_actor_spawn_count(&self) {
    self.actor_spawn_count.add(1, &[]);
  }

  pub fn increment_actor_stopped_count(&self) {
    self.actor_stopped_count.add(1, &[]);
  }

  pub fn increment_dead_letter_count(&self) {
    self.dead_letter_count.add(1, &[]);
  }

  pub fn increment_futures_started_count(&self) {
    self.futures_started_count.add(1, &[]);
  }

  pub fn increment_futures_completed_count(&self) {
    self.futures_completed_count.add(1, &[]);
  }

  pub fn increment_futures_timed_out_count(&self) {
    self.futures_timed_out_count.add(1, &[]);
  }

  pub fn record_thread_pool_latency(&self, latency: f64) {
    self.thread_pool_latency.record(latency, &[]);
  }

  pub fn set_actor_mailbox_length_gauge(&mut self, gauge: ObservableGauge<i64>) {
    self.actor_mailbox_length = gauge;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use opentelemetry_sdk::metrics::ManualReader;

  #[test]
  fn test_actor_metrics() {
    let reader = ManualReader::builder().build();
    let metrics = ActorMetrics::new(reader).expect("メトリクスの初期化に失敗しました");

    // メトリクスの操作をグループ化
    metrics.increment_actor_failure_count();
    metrics.increment_actor_restarted_count();
    metrics.increment_actor_spawn_count();
    metrics.increment_actor_stopped_count();
    metrics.increment_dead_letter_count();
    metrics.increment_futures_started_count();
    metrics.increment_futures_completed_count();
    metrics.increment_futures_timed_out_count();

    // 値を設定するメトリクスをグループ化
    metrics.set_actor_mailbox_length(5);
    metrics.record_actor_message_receive_duration(0.1);
    metrics.record_thread_pool_latency(0.05);

    // 検証可能な操作の結果をアサート
    assert_eq!(
      metrics.get_actor_mailbox_length(),
      5,
      "メールボックスの長さが正しく設定されていません"
    );

    // 他のメトリクスの操作が成功したことを暗黙的に確認
  }
}
