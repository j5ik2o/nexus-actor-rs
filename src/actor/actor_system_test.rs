use async_trait::async_trait;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

use crate::actor::actor::actor_produce_func::ActorProduceFunc;
use crate::actor::actor::props::Props;
use crate::actor::actor::{Actor, ActorError, ActorHandle};
use crate::actor::actor_system::{ActorSystem, Config};
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::{InfoPart, MessagePart, SenderPart, SpawnerPart};
use crate::actor::message::message_handle::{Message, MessageHandle};
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
use crate::actor::util::async_barrier::AsyncBarrier;

#[tokio::test]
async fn test_actor_system_new() {
  let system = ActorSystem::new().await;
  let root = system.get_root_context().await;
  assert_eq!(root.get_self().await, None);
}

#[tokio::test]
async fn test_actor_system_new_with_config() {
  let system = ActorSystem::new_with_config(Config::default()).await;
  let root = system.get_root_context().await;
  assert_eq!(root.get_self().await, None);
}

#[derive(Debug, Clone)]
struct Hello(pub String);
impl Message for Hello {
  fn eq_message(&self, other: &dyn Message) -> bool {
    self.0 == other.as_any().downcast_ref::<Hello>().unwrap().0
  }

  fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug)]
struct MyActor {
  b: AsyncBarrier,
}

#[async_trait]
impl Actor for MyActor {

  async fn receive(&mut self, _: ContextHandle, msg: MessageHandle) -> Result<(), ActorError> {
    println!("{:?}", msg);
    self.b.wait().await;
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[tokio::test]
async fn test_actor_system_spawn_actor() {
  let _ = env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let b = AsyncBarrier::new(2);
  let cloned_b = b.clone();
  let system = ActorSystem::new().await;
  let mut root_context = system.get_root_context().await;

  let props = Props::from_producer_func(
    ActorProduceFunc::new(move |_| {
      let cloned_b = b.clone();
      async move { ActorHandle::new(MyActor {b: cloned_b.clone() }) }
    })
  )
  .await;

  let pid = root_context.spawn(props).await;
  root_context.send(pid, MessageHandle::new(Hello("hello".to_string()))).await;

  cloned_b.wait().await;

}
