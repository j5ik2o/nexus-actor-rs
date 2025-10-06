use std::env;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::join_all;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
#[cfg(feature = "tokio-console")]
use nexus_actor_std_rs::actor::context::lock_wait_snapshot;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, Props};
use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::telemetry;
use nexus_actor_std_rs::Message;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct BurstConfig {
  iterations: usize,
  watchers_per_iteration: usize,
  receive_timeout_ms: u64,
}

#[derive(Debug)]
struct NoopActor;

#[async_trait]
impl Actor for NoopActor {
  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Debug, Clone)]
struct LockProbeActor {
  worker_props: Props,
}

#[async_trait]
impl Actor for LockProbeActor {
  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let handle = ctx.get_message_handle_opt().await.expect("message not found");
    if let Some(config) = handle.to_typed::<BurstConfig>() {
      let timeout = Duration::from_millis(config.receive_timeout_ms);
      for iteration in 0..config.iterations {
        ctx.set_receive_timeout(&timeout).await;
        ctx.cancel_receive_timeout().await;

        for _ in 0..config.watchers_per_iteration {
          let child = ctx.spawn(self.worker_props.clone()).await;
          ctx.watch(&child).await;
          ctx.unwatch(&child).await;
          ctx.stop(&child).await;
        }

        info!(iteration, "lock probe cycle complete");
      }
    }

    Ok(())
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  telemetry::init_console_subscriber()?;

  let iterations = env::var("LOCK_TRACE_ITERATIONS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(256);
  let watchers_per_iteration = env::var("LOCK_TRACE_WATCHERS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(8);
  let receive_timeout_ms = env::var("LOCK_TRACE_TIMEOUT_MS")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(5);
  let parallel = env::var("LOCK_TRACE_PARALLEL")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(4);

  let config = BurstConfig {
    iterations,
    watchers_per_iteration,
    receive_timeout_ms,
  };

  let system = ActorSystem::new().await?;
  let mut root = system.get_root_context().await;

  let worker_props = Props::from_async_actor_producer(|_| async { NoopActor }).await;
  let worker_props_for_probe = worker_props.clone();

  let load_props = Props::from_async_actor_producer(move |_| {
    let worker_props = worker_props_for_probe.clone();
    async move { LockProbeActor { worker_props } }
  })
  .await;

  let load_pid = root.spawn(load_props).await;

  let mut tasks = Vec::with_capacity(parallel);
  for idx in 0..parallel {
    let pid = load_pid.clone();
    let payload = config.clone();
    let ctx_clone = root.clone();
    tasks.push(tokio::spawn(async move {
      let mut ctx = ctx_clone;
      info!(batch = idx, "dispatching burst message");
      ctx.send(pid, MessageHandle::new(payload)).await;
    }));
  }

  join_all(tasks).await;

  tokio::time::sleep(Duration::from_secs(5)).await;

  #[cfg(feature = "tokio-console")]
  {
    let mut stats = lock_wait_snapshot();
    stats.sort_by(|a, b| b.max_wait_ns.cmp(&a.max_wait_ns));
    for record in stats.iter().take(8) {
      info!(
        target: "actor_context_lock_tracing::summary",
        component = record.component,
        operation = record.operation,
        mode = record.mode,
        samples = record.samples,
        max_wait_us = record.max_wait_ns as f64 / 1000.0,
        avg_wait_us = record.avg_wait_ns() / 1000.0,
      );
    }
  }

  Ok(())
}
