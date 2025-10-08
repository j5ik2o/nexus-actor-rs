//! Dispatcher 経路でのメールボックス滞留時間を計測するベンチマーク。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use nexus_actor_std_rs::actor::core::{ActorError, ErrorReason};
use nexus_actor_std_rs::actor::dispatch::dispatcher::{CoreSchedulerDispatcher, DispatcherHandle};
use nexus_actor_std_rs::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use nexus_actor_std_rs::actor::dispatch::{unbounded_mpsc_mailbox_creator, Mailbox, MailboxHandle, MailboxQueueKind};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_actor_std_rs::runtime::tokio_core_runtime;
use std::hint::black_box;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, RwLock};
use tokio::task::yield_now;

#[derive(Debug, Clone, PartialEq, ::nexus_message_derive_rs::Message)]
struct BenchPayload {
  seq: usize,
}

#[derive(Debug)]
struct RecordingInvoker {
  process_delay: Duration,
  processed: Arc<AtomicUsize>,
  latencies_ms: Arc<Mutex<Vec<f64>>>,
}

impl RecordingInvoker {
  fn new(process_delay: Duration, processed: Arc<AtomicUsize>, latencies_ms: Arc<Mutex<Vec<f64>>>) -> Self {
    Self {
      process_delay,
      processed,
      latencies_ms,
    }
  }
}

#[async_trait::async_trait]
impl MessageInvoker for RecordingInvoker {
  async fn invoke_system_message(&mut self, _message_handle: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn invoke_user_message(&mut self, _message_handle: MessageHandle) -> Result<(), ActorError> {
    if !self.process_delay.is_zero() {
      tokio::time::sleep(self.process_delay).await;
    }
    self.processed.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  async fn escalate_failure(&mut self, _reason: ErrorReason, _message_handle: MessageHandle) {}

  async fn record_mailbox_queue_latency(&mut self, queue: MailboxQueueKind, latency: Duration) {
    if queue == MailboxQueueKind::User {
      let mut guard = self.latencies_ms.lock().await;
      guard.push(latency.as_secs_f64() * 1_000.0);
    }
  }
}

async fn run_dispatch_cycle(load: usize, process_delay: Duration) -> f64 {
  let processed = Arc::new(AtomicUsize::new(0));
  let latencies_ms = Arc::new(Mutex::new(Vec::with_capacity(load)));

  let invoker = Arc::new(RwLock::new(RecordingInvoker::new(
    process_delay,
    processed.clone(),
    latencies_ms.clone(),
  )));

  let dispatcher = Arc::new(CoreSchedulerDispatcher::from_runtime(tokio_core_runtime()));
  let producer = unbounded_mpsc_mailbox_creator();
  let mut mailbox: MailboxHandle = producer.run().await;
  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new_with_metrics(invoker.clone(), true)),
      Some(DispatcherHandle::new_arc(dispatcher.clone())),
    )
    .await;
  mailbox.start().await;

  for seq in 0..load {
    mailbox
      .post_user_message(MessageHandle::new(BenchPayload { seq }))
      .await;
  }

  while processed.load(Ordering::SeqCst) < load {
    yield_now().await;
  }

  let latencies = latencies_ms.lock().await;
  if latencies.is_empty() {
    0.0
  } else {
    latencies.iter().copied().sum::<f64>() / latencies.len() as f64
  }
}

fn dispatcher_queue_latency_bench(c: &mut Criterion) {
  let factory = Runtime::new().expect("tokio runtime");
  let mut group = c.benchmark_group("dispatcher_queue_latency");
  group.sample_size(10);
  group.warm_up_time(Duration::from_millis(500));
  group.measurement_time(Duration::from_secs(3));

  let loads = [128usize, 1024usize, 4096usize];
  for &load in &loads {
    group.throughput(Throughput::Elements(load as u64));
    group.bench_function(format!("user_queue_dwell_load_{}", load), |b| {
      b.to_async(&runtime)
        .iter(|| async move { black_box(run_dispatch_cycle(load, Duration::from_micros(40)).await) });
    });
  }

  group.finish();
}

criterion_group!(benches, dispatcher_queue_latency_bench);
criterion_main!(benches);
