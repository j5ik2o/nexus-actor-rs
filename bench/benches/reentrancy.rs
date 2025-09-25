use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use criterion::{criterion_group, criterion_main, Criterion};
use futures::stream::{self, StreamExt};
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_core_rs::actor::message::Message;
use nexus_actor_core_rs::actor::message::{MessageHandle, ResponseHandle};
use nexus_actor_core_rs::actor::process::process_registry::AddressResolver;
use nexus_actor_core_rs::actor::{ConfigOption, MetricsProvider};
use nexus_actor_core_rs::generated::actor::Pid;
use opentelemetry::metrics::noop::NoopMeterProvider;
use rand::random;
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, Instant};

#[derive(Debug, Clone, PartialEq, ::nexus_actor_message_derive_rs::Message)]
struct LoadMessage {
  seq: u64,
}

#[derive(Debug, Clone, PartialEq, ::nexus_actor_message_derive_rs::Message)]
struct LoadReply {
  seq: u64,
}

#[derive(Debug, Clone, PartialEq, ::nexus_actor_message_derive_rs::Message)]
struct LoadAck {
  seq: u64,
}

#[derive(Debug, Clone)]
struct LoadActor;

#[async_trait::async_trait]
impl Actor for LoadActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    if let Some(msg) = ctx.get_message_handle().await.to_typed::<LoadMessage>() {
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

#[derive(Debug, Clone, PartialEq, ::nexus_actor_message_derive_rs::Message)]
struct BorrowRequest {
  seq: u64,
}

#[derive(Debug, Clone, PartialEq, ::nexus_actor_message_derive_rs::Message)]
struct BorrowResponse {
  seq: u64,
}

#[derive(Debug, Clone)]
struct BorrowingActor;

#[async_trait::async_trait]
impl Actor for BorrowingActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    if let Some(message) = ctx.get_message_handle().await.to_typed::<BorrowRequest>() {
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
  let system = ActorSystem::new_config_options([ConfigOption::SetMetricsProvider(Arc::new(MetricsProvider::Noop(
    NoopMeterProvider::default(),
  )))])
  .await
  .expect("init actor system");

  let registry = system.get_process_registry().await;
  let failure_ratio = failure_ratio.clamp(0.0, 1.0);

  registry.register_address_resolver(AddressResolver::new({
    let system = system.clone();
    move |_pid: &ExtendedPid| {
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
  let runtime = Builder::new_multi_thread().enable_all().build().expect("runtime");
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
  let runtime = Builder::new_multi_thread().enable_all().build().expect("runtime");
  let total_requests = 2_000;

  group.bench_function("borrow_hot_path", |b| {
    b.iter_custom(|iters| {
      runtime.block_on(async {
        let mut total = StdDuration::ZERO;
        for _ in 0..iters {
          let start = Instant::now();
          run_context_borrow_scenario(total_requests).await;
          total += start.elapsed();
        }
        total
      })
    });
  });
  group.finish();
}

criterion_group!(benches, reentrancy_benchmark, context_borrow_benchmark);
criterion_main!(benches);
