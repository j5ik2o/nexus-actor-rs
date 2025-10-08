//! ActorContext 周辺で発生する dispatcher / mailbox 滞留を計測するベンチマーク。

use std::any::Any;
use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_utils_std_rs::collections::{MpscUnboundedChannelQueue, QueueReader, QueueWriter};
use tokio::runtime::Runtime;
use tokio::task::yield_now;
use tokio::time::Instant;

#[derive(Clone, Debug)]
struct TimestampedMessage {
  created_at: Instant,
  seq: u64,
}

impl nexus_actor_std_rs::actor::message::Message for TimestampedMessage {
  fn eq_message(&self, other: &dyn nexus_actor_std_rs::actor::message::Message) -> bool {
    other
      .as_any()
      .downcast_ref::<TimestampedMessage>()
      .map(|rhs| rhs.seq == self.seq)
      .unwrap_or(false)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "bench::TimestampedMessage".into()
  }
}

async fn run_enq_deq_cycle(load: usize, process_delay: Duration) -> f64 {
  let queue = MpscUnboundedChannelQueue::<MessageHandle>::new();
  let mut writer = queue.clone();
  let mut reader = queue.clone();

  let consumer = tokio::spawn(async move {
    let mut latencies = Vec::with_capacity(load);
    let mut processed = 0usize;
    while processed < load {
      match reader.poll() {
        Ok(Some(message)) => {
          if let Some(ts) = message.to_typed::<TimestampedMessage>() {
            let waited = ts.created_at.elapsed();
            latencies.push(waited.as_secs_f64() * 1_000.0);
          }
          processed += 1;
          tokio::time::sleep(process_delay).await;
        }
        Ok(None) => {
          yield_now().await;
        }
        Err(_) => break,
      }
    }
    latencies
  });

  for seq in 0..load {
    let message = MessageHandle::new(TimestampedMessage {
      created_at: Instant::now(),
      seq: seq as u64,
    });
    writer.offer(message).expect("offer should succeed");
  }

  let latencies = consumer.await.expect("consumer task panicked");
  if latencies.is_empty() {
    0.0
  } else {
    latencies.iter().copied().sum::<f64>() / latencies.len() as f64
  }
}

fn actor_context_lock_bench(c: &mut Criterion) {
  let factory = Runtime::new().expect("tokio runtime");
  let mut group = c.benchmark_group("actor_dispatch_queue");
  group.sample_size(10);
  group.warm_up_time(Duration::from_millis(500));
  group.measurement_time(Duration::from_secs(3));
  let loads = [128usize, 1024usize, 4096usize];
  for &load in &loads {
    group.throughput(Throughput::Elements(load as u64));
    group.bench_function(format!("enqueue_dequeue_load_{}", load), |b| {
      b.to_async(&runtime)
        .iter(|| async move { black_box(run_enq_deq_cycle(load, Duration::from_micros(50)).await) });
    });
  }
  group.finish();
}

criterion_group!(benches, actor_context_lock_bench);
criterion_main!(benches);
