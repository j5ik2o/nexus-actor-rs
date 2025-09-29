//! DelayQueue を用いた receive_timeout デモ (PoC)

use std::env;
use std::time::Duration;

use futures::StreamExt;
use tokio_util::time::delay_queue::{DelayQueue, Key};
use tracing::info;

struct DelayQueueTimer {
  queue: DelayQueue<()>,
  key: Option<Key>,
}

impl DelayQueueTimer {
  fn new(duration: Duration) -> Self {
    let mut queue = DelayQueue::new();
    let key = queue.insert((), duration);
    Self { queue, key: Some(key) }
  }

  fn reset(&mut self, duration: Duration) {
    match self.key {
      Some(ref key) => self.queue.reset(key, duration),
      None => {
        self.key = Some(self.queue.insert((), duration));
      }
    }
  }

  fn stop(&mut self) {
    if let Some(key) = self.key.take() {
      let _ = self.queue.remove(&key);
    }
  }

  async fn wait(&mut self) {
    if self.key.is_none() {
      return;
    }
    let _ = self.queue.next().await;
  }
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .init();

  let iterations = env::var("DELAYQUEUE_ITERATIONS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(10_000u32);
  let duration = env::var("DELAYQUEUE_TIMEOUT_MS")
    .ok()
    .and_then(|v| v.parse().ok())
    .map(Duration::from_millis)
    .unwrap_or(Duration::from_millis(25));

  let mut timer = DelayQueueTimer::new(duration);
  let start = tokio::time::Instant::now();
  for _ in 0..iterations {
    timer.reset(duration);
  }
  let rearm_elapsed = start.elapsed();

  timer.stop();

  let mut timer = DelayQueueTimer::new(duration);
  let wait_start = tokio::time::Instant::now();
  timer.wait().await;
  let fire_elapsed = wait_start.elapsed();

  info!(
    iterations,
    timeout_ms = duration.as_millis(),
    rearm_total_ms = rearm_elapsed.as_secs_f64() * 1000.0,
    rearm_avg_ns = rearm_elapsed.as_nanos() as f64 / iterations as f64,
    fire_elapsed_ms = fire_elapsed.as_secs_f64() * 1000.0,
    "delay_queue receive_timeout baseline"
  );

  println!(
    "DelayQueue baseline: iterations={iterations}, timeout_ms={}, rearm_total_ms={:.3}, rearm_avg_ns={:.1}, fire_elapsed_ms={:.3}",
    duration.as_millis(),
    rearm_elapsed.as_secs_f64() * 1000.0,
    rearm_elapsed.as_nanos() as f64 / iterations as f64,
    fire_elapsed.as_secs_f64() * 1000.0
  );
}
