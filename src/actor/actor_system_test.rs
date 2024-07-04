use std::thread::sleep;

use async_trait::async_trait;

use crate::actor::actor::actor_produce_func::ActorProduceFunc;
use crate::actor::actor::props::Props;
use crate::actor::actor::{Actor, ActorError, ActorHandle};
use crate::actor::actor_system::{ActorSystem, Config};
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::{InfoPart, MessagePart, SenderPart, SpawnerPart};
use crate::actor::message::message_handle::{Message, MessageHandle};
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

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
struct MyActor {}

#[async_trait]
impl Actor for MyActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let msg = ctx.get_message().await;
    println!("{:?}", msg);
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

pub async fn receive(_: ContextHandle) -> ActorHandle {
  let actor = MyActor {};
  ActorHandle::new(actor)
}
fn init() {
  let _ = env_logger::builder().is_test(true).try_init();
}

// spawn actor test
#[tokio::test]
async fn test_actor_system_spawn_actor() {
  init();
  let system = ActorSystem::new().await;
  let mut root = system.get_root_context().await;
  log::debug!("root: {:?}", root);
  let props = Props::from_producer_func_with_opts(ActorProduceFunc::new(receive), vec![]).await;
  log::debug!("props: {:?}", props);
  let pid = root.spawn(props).await;
  log::debug!("pid: {:?}", pid);
  println!("pid: {:?}", pid);
  root.send(pid, MessageHandle::new(Hello("hello".to_string()))).await;

  sleep(std::time::Duration::from_secs(3));
}
