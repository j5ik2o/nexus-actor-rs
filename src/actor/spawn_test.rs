use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Notify;
use tracing_subscriber::EnvFilter;

use crate::actor::actor::actor_produce_func::ActorProduceFunc;
use crate::actor::actor::props::Props;
use crate::actor::actor::{Actor, ActorError, ActorHandle};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::SpawnerPart;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

#[derive(Debug, Clone)]
struct MyActor {
  is_started: Arc<AtomicBool>,
  received: Arc<Notify>,
}

#[async_trait]
impl Actor for MyActor {
  async fn started(&self, _: ContextHandle) -> Result<(), ActorError> {
    println!("MyActor started");
    self.is_started.store(true, Ordering::SeqCst);
    self.received.notify_one();
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[tokio::test]
async fn test_example() {
  let _ = env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await;

  let actor = MyActor {
    is_started: Arc::new(AtomicBool::new(false)),
    received: Arc::new(Notify::new()),
  };

  let producer_func = ActorProduceFunc::new({
    let actor = actor.clone();
    move |_| {
      let actor = actor.clone();
      async move { ActorHandle::new(actor.clone()) }
    }
  });

  let props = Props::from_producer_func(producer_func).await;

  system.get_root_context().await.spawn(props).await;

  actor.received.notified().await;

  assert_eq!(actor.is_started.load(Ordering::SeqCst), true);
}
