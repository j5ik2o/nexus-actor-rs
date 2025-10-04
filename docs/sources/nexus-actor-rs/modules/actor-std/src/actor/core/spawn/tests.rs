use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tokio::time::{sleep, Duration};
use tracing_subscriber::EnvFilter;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ContextHandle, SenderContextHandle, SpawnerPart, StopperPart};
use crate::actor::core::actor::Actor;
use crate::actor::core::actor_error::ActorError;
use crate::actor::core::pid::ExtendedPid;
use crate::actor::core::props::Props;
use crate::actor::core::sender_middleware::SenderMiddleware;
use crate::actor::core::sender_middleware_chain::SenderMiddlewareChain;
use crate::actor::core::spawner::Spawner;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::SupervisorStrategyHandle;
use nexus_actor_core_rs::context::core::{CoreSenderInvocation, CoreSpawnInvocation};
use nexus_actor_core_rs::{CoreMessageEnvelope, CorePid};

#[derive(Debug, Clone)]
struct MyActor {
  is_started: Arc<AtomicBool>,
  received: Arc<Notify>,
}

#[async_trait]
impl Actor for MyActor {
  async fn post_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("MyActor started");
    self.is_started.store(true, Ordering::SeqCst);
    self.received.notify_one();
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Debug, Clone)]
struct ParentActor {
  handle_slot: Arc<AsyncMutex<Option<ContextHandle>>>,
  ready: Arc<Notify>,
}

#[async_trait]
impl Actor for ParentActor {
  async fn post_start(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    {
      let mut guard = self.handle_slot.lock().await;
      *guard = Some(ctx.clone());
    }
    self.ready.notify_one();
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Debug, Clone)]
struct ChildActor {
  received: Arc<AtomicBool>,
}

#[async_trait]
impl Actor for ChildActor {
  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    self.received.store(true, Ordering::SeqCst);
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[tokio::test]
async fn test_example() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();

  let actor = MyActor {
    is_started: Arc::new(AtomicBool::new(false)),
    received: Arc::new(Notify::new()),
  };

  let actor_producer = {
    let actor = actor.clone();
    move |_| {
      let actor = actor.clone();
      async move { actor.clone() }
    }
  };

  let props = Props::from_async_actor_producer(actor_producer).await;

  let pid = system.get_root_context().await.spawn(props).await;

  tracing::info!("pid = {:?}", pid);

  actor.received.notified().await;

  assert!(actor.is_started.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_spawn_adapter_core_invocation_chain() {
  let system = ActorSystem::new().await.unwrap();

  let handle_slot = Arc::new(AsyncMutex::new(None));
  let ready = Arc::new(Notify::new());

  let parent_props = Props::from_sync_actor_producer_with_opts(
    {
      let handle_slot = handle_slot.clone();
      let ready = ready.clone();
      move |_| ParentActor {
        handle_slot: handle_slot.clone(),
        ready: ready.clone(),
      }
    },
    [],
  )
  .await;

  let mut root = system.get_root_context().await;
  let parent_pid = root.spawn(parent_props.clone()).await;

  ready.notified().await;
  let context_handle = {
    let guard = handle_slot.lock().await;
    guard.clone().expect("parent context handle not captured")
  };

  let sender_handle = SenderContextHandle::from_context(context_handle.clone());
  let parent_snapshot = sender_handle
    .capture_core_snapshot()
    .await
    .expect("failed to capture sender snapshot");

  let spawn_flag = Arc::new(AtomicBool::new(false));
  let sender_flag = Arc::new(AtomicBool::new(false));
  let child_received = Arc::new(AtomicBool::new(false));
  let spawn_middleware = crate::actor::core::spawn_middleware::SpawnMiddleware::new({
    let spawn_flag = spawn_flag.clone();
    move |next| {
      let spawn_flag = spawn_flag.clone();
      Spawner::new(move |system, name, props, parent| {
        let spawn_flag = spawn_flag.clone();
        let next = next.clone();
        async move {
          spawn_flag.store(true, Ordering::SeqCst);
          next.run(system, &name, props, parent).await
        }
      })
    }
  });

  let sender_middleware = SenderMiddleware::new({
    let sender_flag = sender_flag.clone();
    move |chain: SenderMiddlewareChain| {
      SenderMiddlewareChain::new({
        let sender_flag = sender_flag.clone();
        let chain = chain.clone();
        move |context, target_core, envelope| {
          let sender_flag = sender_flag.clone();
          let chain = chain.clone();
          async move {
            sender_flag.store(true, Ordering::SeqCst);
            let target = ExtendedPid::from_core(target_core.clone());
            chain.run(context, target, envelope).await;
          }
        }
      })
    }
  });

  let child_props = Props::from_sync_actor_producer_with_opts(
    {
      let child_received = child_received.clone();
      move |_| ChildActor {
        received: child_received.clone(),
      }
    },
    [
      Props::with_spawn_middleware([spawn_middleware]),
      Props::with_sender_middlewares([sender_middleware]),
    ],
  )
  .await;

  let core_props = child_props.core_props();
  let adapter = core_props.spawn_adapter().expect("missing spawn adapter");
  let spawn_chain = core_props
    .spawn_middleware_chain()
    .cloned()
    .expect("missing spawn chain");

  let address = system.get_address().await;
  let reserved_id = system.get_process_registry().await.next_id();
  let reserved_pid = CorePid::new(address, reserved_id.clone());
  let metadata = child_props.actor_type_hint();

  let invocation = CoreSpawnInvocation::new(
    parent_snapshot.clone(),
    core_props.clone(),
    reserved_pid.clone(),
    metadata,
    adapter,
  );

  let result_pid = spawn_chain.call(invocation).await.expect("spawn invocation failed");

  assert_eq!(result_pid, reserved_pid);
  assert!(spawn_flag.load(Ordering::SeqCst));

  // Allow the spawned actor to finish initialization.
  sleep(Duration::from_millis(50)).await;

  if let Some(sender_chain) = core_props.sender_middleware_chain().cloned() {
    let envelope = CoreMessageEnvelope::new(MessageHandle::new(42_i32));
    let sender_invocation = CoreSenderInvocation::new(parent_snapshot.clone(), result_pid.clone(), envelope);
    sender_chain.run(sender_invocation).await;
  }

  sleep(Duration::from_millis(50)).await;

  assert!(sender_flag.load(Ordering::SeqCst));
  assert!(child_received.load(Ordering::SeqCst));

  let extended_child = crate::actor::core::pid::ExtendedPid::from_core(result_pid.clone());
  assert!(
    system
      .get_process_registry()
      .await
      .get_process(&extended_child)
      .await
      .is_some(),
    "child process not registered"
  );

  root.stop(&extended_child).await;
  root.stop(&parent_pid).await;
}
