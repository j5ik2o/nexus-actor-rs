use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use criterion::{criterion_group, criterion_main, Criterion};
use futures::stream::{self, StreamExt};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::actor::message::{MessageHandle, ResponseHandle};
use nexus_actor_std_rs::actor::process::process_registry::AddressResolver;
use nexus_actor_std_rs::actor::{ConfigOption, MetricsProvider};
use nexus_actor_std_rs::generated::actor::Pid;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use rand::random;
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, Instant};

#[cfg(feature = "lock-metrics")]
use nexus_actor_std_rs::actor::context::{
  context_lock_metrics_snapshot, reset_context_lock_metrics, ContextLockMetricsSnapshot,
};
#[cfg(feature = "lock-metrics")]
use std::sync::Mutex as StdMutex;

#[cfg(feature = "lock-metrics")]
#[derive(Debug, Default, Clone, Copy)]
struct LockMetricsAggregate {
  iterations: u64,
  read_lock_acquisitions: u64,
  write_lock_acquisitions: u64,
  snapshot_hits: u64,
  snapshot_misses: u64,
}

#[cfg(feature = "lock-metrics")]
impl LockMetricsAggregate {
  fn add_snapshot(&mut self, snapshot: ContextLockMetricsSnapshot) {
    self.iterations += 1;
    self.read_lock_acquisitions += snapshot.read_lock_acquisitions;
    self.write_lock_acquisitions += snapshot.write_lock_acquisitions;
    self.snapshot_hits += snapshot.snapshot_hits;
    self.snapshot_misses += snapshot.snapshot_misses;
  }
}

#[derive(Debug, Clone, PartialEq, ::nexus_message_derive_rs::Message)]
struct LoadMessage {
  seq: u64,
}

#[derive(Debug, Clone, PartialEq, ::nexus_message_derive_rs::Message)]
struct LoadReply {
  seq: u64,
}

#[derive(Debug, Clone, PartialEq, ::nexus_message_derive_rs::Message)]
struct LoadAck {
  seq: u64,
}

#[derive(Debug, Clone)]
struct LoadActor;

#[async_trait::async_trait]
impl Actor for LoadActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    if let Some(msg) = ctx
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<LoadMessage>()
    {
      let remote_pid = ExtendedPid::new(Pid::new("remote-host", &format!("load-{}", msg.seq)));
      remote_pid
        .send_user_message(
          ctx.get_actor_system().await,
          MessageHandle::new(LoadAck { seq: msg.seq }),
        )
        .await;

      ctx.respond(ResponseHandle::new(LoadReply { seq: msg.seq })).await;
    }
    Ok(())
  }

  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    self.handle(ctx).await
  }
}

#[derive(Debug, Clone, PartialEq, ::nexus_message_derive_rs::Message)]
struct BorrowRequest {
  seq: u64,
}

#[derive(Debug, Clone, PartialEq, ::nexus_message_derive_rs::Message)]
struct BorrowResponse {
  seq: u64,
}

#[derive(Debug, Clone)]
struct BorrowingActor;

#[async_trait::async_trait]
impl Actor for BorrowingActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    if let Some(message) = ctx
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<BorrowRequest>()
    {
      if let Some(actor_context) = ctx.try_into_actor_context().await {
        let borrow = actor_context.borrow();
        let _ = borrow.self_pid();
        let _ = borrow.props();
      }
      ctx
        .respond(ResponseHandle::new(BorrowResponse { seq: message.seq }))
        .await;
    }
    Ok(())
  }

  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    self.handle(ctx).await
  }
}

struct ScenarioMetrics {
  successes: usize,
  timeouts: usize,
}

async fn run_scenario(total_requests: usize, concurrency: usize, failure_ratio: f64) -> ScenarioMetrics {
  let system = ActorSystem::new_config_options([ConfigOption::SetMetricsProvider(Arc::new(MetricsProvider::Sdk(
    SdkMeterProvider::default(),
  )))])
  .await
  .expect("init actor system");

  let registry = system.get_process_registry().await;
  let failure_ratio = failure_ratio.clamp(0.0, 1.0);

  registry.register_address_resolver(AddressResolver::new({
    let system = system.clone();
    move |_pid: &CorePid| {
      let system = system.clone();
      async move {
        if random::<f64>() < failure_ratio {
          None
        } else {
          Some(system.get_dead_letter().await)
        }
      }
    }
  }));

  let mut root_context = system.get_root_context().await;
  let load_pid = root_context
    .spawn(Props::from_async_actor_producer(|_| async { LoadActor }).await)
    .await;
  let root_context = root_context.clone();

  let durations = Arc::new(Mutex::new(Vec::with_capacity(total_requests)));
  let timeout_count = Arc::new(AtomicUsize::new(0));

  stream::iter(0..total_requests)
    .for_each_concurrent(concurrency, |seq| {
      let root_context = root_context.clone();
      let load_pid = load_pid.clone();
      let durations = Arc::clone(&durations);
      let timeout_count = Arc::clone(&timeout_count);
      async move {
        let message = MessageHandle::new(LoadMessage { seq: seq as u64 });
        let timeout_duration = Duration::from_millis(200);
        let result = root_context
          .request_future(load_pid.clone(), message, timeout_duration)
          .await;

        let request_start = Instant::now();
        match timeout(timeout_duration, result.result()).await {
          Ok(Ok(_reply)) => {
            let elapsed = request_start.elapsed().as_secs_f64() * 1_000.0;
            let mut guard = durations.lock().await;
            guard.push(elapsed);
          }
          _ => {
            timeout_count.fetch_add(1, Ordering::Relaxed);
          }
        }
      }
    })
    .await;

  let successes = {
    let guard = durations.lock().await;
    guard.len()
  };

  ScenarioMetrics {
    successes,
    timeouts: timeout_count.load(Ordering::Relaxed),
  }
}

fn reentrancy_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("reentrancy");
  let factory = Builder::new_multi_thread().enable_all().build().expect("runtime");
  let total_requests = 2_000;
  let concurrency = 64;
  let failure_ratio = 0.5;

  group.bench_function("load", |b| {
    b.iter_custom(|iters| {
      runtime.block_on(async {
        let mut total = StdDuration::ZERO;
        for _ in 0..iters {
          let start = Instant::now();
          let metrics = run_scenario(total_requests, concurrency, failure_ratio).await;
          assert_eq!(metrics.successes + metrics.timeouts, total_requests);
          total += start.elapsed();
        }
        total
      })
    });
  });
  group.finish();
}

async fn run_context_borrow_scenario(total_requests: usize) {
  let system = ActorSystem::new().await.expect("init actor system");
  let mut root_context = system.get_root_context().await;
  let borrow_pid = root_context
    .spawn(Props::from_async_actor_producer(|_| async { BorrowingActor }).await)
    .await;
  let timeout_duration = Duration::from_millis(150);

  for seq in 0..total_requests {
    let request = MessageHandle::new(BorrowRequest { seq: seq as u64 });
    let future = root_context
      .request_future(borrow_pid.clone(), request, timeout_duration)
      .await;
    let reply = future.result().await.expect("borrow actor future should succeed");
    assert!(reply.to_typed::<BorrowResponse>().is_some());
  }
}

fn context_borrow_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("context_borrow");
  let factory = Builder::new_multi_thread().enable_all().build().expect("runtime");
  let total_requests = 2_000;

  #[cfg(feature = "lock-metrics")]
  let lock_metrics = Arc::new(StdMutex::new(LockMetricsAggregate::default()));

  group.bench_function("borrow_hot_path", |b| {
    #[cfg(feature = "lock-metrics")]
    let lock_metrics = Arc::clone(&lock_metrics);
    b.iter_custom(|iters| {
      #[cfg(feature = "lock-metrics")]
      let lock_metrics = Arc::clone(&lock_metrics);
      runtime.block_on(async {
        let mut total = StdDuration::ZERO;
        for _ in 0..iters {
          #[cfg(feature = "lock-metrics")]
          reset_context_lock_metrics();
          let start = Instant::now();
          run_context_borrow_scenario(total_requests).await;
          total += start.elapsed();
          #[cfg(feature = "lock-metrics")]
          {
            let snapshot = context_lock_metrics_snapshot();
            let mut guard = lock_metrics.lock().unwrap();
            guard.add_snapshot(snapshot);
          }
        }
        total
      })
    });
  });
  #[cfg(feature = "lock-metrics")]
  {
    let metrics = lock_metrics.lock().unwrap().clone();
    if metrics.iterations > 0 {
      let total_requests_count = metrics.iterations * total_requests as u64;
      let read_per_iter = metrics.read_lock_acquisitions as f64 / metrics.iterations as f64;
      let write_per_iter = metrics.write_lock_acquisitions as f64 / metrics.iterations as f64;
      println!(
        "[lock-metrics] borrow_hot_path: iterations={}, total_requests={}, read_locks={}, write_locks={}, snapshot_hits={}, snapshot_misses={}, read_locks_per_iter={:.2}, write_locks_per_iter={:.2}",
        metrics.iterations,
        total_requests_count,
        metrics.read_lock_acquisitions,
        metrics.write_lock_acquisitions,
        metrics.snapshot_hits,
        metrics.snapshot_misses,
        read_per_iter,
        write_per_iter
      );
    } else {
      println!("[lock-metrics] borrow_hot_path: no iterations recorded");
    }
  }
  group.finish();
}

criterion_group!(benches, reentrancy_benchmark, context_borrow_benchmark);
criterion_main!(benches);
