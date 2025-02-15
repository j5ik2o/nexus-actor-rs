use crate::actor::MetricsProvider;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::metrics::{Counter, Histogram, Meter};
use opentelemetry::KeyValue;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const LIB_NAME: &str = "protoactor";

#[derive(Debug, Clone)]
struct ActorMetricsInner {
  meter: Meter,
  actor_failure_count: Counter<u64>,
  actor_mailbox_length: Counter<u64>,
  actor_message_receive_histogram: Histogram<f64>,
  actor_restarted_count: Counter<u64>,
  actor_spawn_count: Counter<u64>,
  actor_stopped_count: Counter<u64>,
  dead_letter_count: Counter<u64>,
  futures_started_count: Counter<u64>,
  futures_completed_count: Counter<u64>,
  futures_timed_out_count: Counter<u64>,
  thread_pool_latency: Histogram<f64>,
}

#[derive(Debug, Clone)]
pub struct ActorMetrics {
  inner: Arc<Mutex<ActorMetricsInner>>,
}

impl ActorMetrics {
  pub fn new(meter_provider: Arc<MetricsProvider>) -> Result<Self, Box<dyn std::error::Error>> {
    let meter = meter_provider.meter(LIB_NAME);

    Ok(ActorMetrics {
      inner: Arc::new(Mutex::new(ActorMetricsInner {
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
        // mailbox_length,
      })),
    })
  }

  pub async fn increment_actor_failure_count(&self) {
    self.increment_actor_failure_count_with_opts(&[]).await;
  }

  pub async fn increment_actor_failure_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_failure_count.add(1, attributes);
  }

  pub async fn increment_actor_mailbox_length(&self) {
    self.increment_actor_mailbox_length_with_opts(&[]).await;
  }

  pub async fn increment_actor_mailbox_length_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_mailbox_length.add(1, attributes);
  }

  pub async fn record_actor_message_receive_duration(&self, duration: f64) {
    self
      .record_actor_message_receive_duration_with_opts(duration, &[])
      .await;
  }

  pub async fn record_actor_message_receive_duration_with_opts(&self, duration: f64, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_message_receive_histogram.record(duration, attributes);
  }

  pub async fn increment_actor_restarted_count(&self) {
    self.increment_actor_restarted_count_with_opts(&[]).await;
  }

  pub async fn increment_actor_restarted_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_restarted_count.add(1, attributes);
  }

  pub async fn increment_actor_spawn_count(&self) {
    self.increment_actor_spawn_count_with_opts(&[]).await;
  }

  pub async fn increment_actor_spawn_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_spawn_count.add(1, attributes);
  }

  pub async fn increment_actor_stopped_count(&self) {
    self.increment_actor_stopped_count_with_opts(&[]).await;
  }

  pub async fn increment_actor_stopped_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_stopped_count.add(1, attributes);
  }

  pub async fn increment_dead_letter_count(&self) {
    self.increment_dead_letter_count_with_opts(&[]).await;
  }

  pub async fn increment_dead_letter_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.dead_letter_count.add(1, attributes);
  }

  pub async fn increment_futures_started_count(&self) {
    self.increment_futures_started_count_with_opts(&[]).await
  }

  pub async fn increment_futures_started_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.futures_started_count.add(1, attributes);
  }

  pub async fn increment_futures_completed_count(&self) {
    self.increment_futures_completed_count_with_opts(&[]).await;
  }

  pub async fn increment_futures_completed_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.futures_completed_count.add(1, attributes);
  }

  pub async fn increment_futures_timed_out_count(&self) {
    self.increment_futures_timed_out_count_with_opts(&[]).await;
  }

  pub async fn increment_futures_timed_out_count_with_opts(&self, attributes: &[KeyValue]) {
    let inner_mg = self.inner.lock().await;
    inner_mg.futures_timed_out_count.add(1, attributes);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use opentelemetry_sdk::metrics::{ManualReader, MeterProviderBuilder};

  #[tokio::test]
  async fn test_actor_metrics() {
    let reader = ManualReader::builder().build();
    let meter_provider = MeterProviderBuilder::default().with_reader(reader).build();
    let meter_provider = MetricsProvider::Sdk(meter_provider);
    let metrics = ActorMetrics::new(Arc::new(meter_provider)).expect("メトリクスの初期化に失敗しました");

    metrics.increment_actor_failure_count().await;
    metrics.increment_actor_mailbox_length().await;
    metrics.increment_actor_restarted_count().await;
    metrics.increment_actor_spawn_count().await;
    metrics.increment_actor_stopped_count().await;
    metrics.increment_dead_letter_count().await;
    metrics.increment_futures_started_count().await;
    metrics.increment_futures_completed_count().await;
    metrics.increment_futures_timed_out_count().await;
  }
}
