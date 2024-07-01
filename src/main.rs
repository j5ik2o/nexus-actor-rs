use std::sync::Arc;
use std::thread::sleep;

use async_trait::async_trait;

use nexus_rs::actor::actor::props::Props;
use nexus_rs::actor::actor::{Actor, ActorError, ActorHandle};
use nexus_rs::actor::actor_system::ActorSystem;
use nexus_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_rs::actor::dispatch::unbounded::unbounded_mpsc_mailbox_creator;
use nexus_rs::actor::message::{Message, MessageHandle, ProducerFunc};
use nexus_rs::actor::messages::SystemMessage;

#[derive(Debug, Clone)]
struct Hello(pub String);

impl Message for Hello {
  fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug)]
struct ChildActor {}

#[async_trait]
impl Actor for ChildActor {
  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    println!("ChildActor::post_start");
    Ok(())
  }

  async fn receive(&self, _: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
    println!("ChildActor::receive: msg = {:?}", message_handle);
    Ok(())
  }
}

#[derive(Debug)]
struct TopActor {}

#[async_trait]
impl Actor for TopActor {
  async fn post_start(&self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    println!("TopActor::post_start");
    let props = Props::from_producer_func(ProducerFunc::new(create_child_actor)).await;

    let pid = context_handle.spawn(props).await;
    for _ in 1..10 {
      context_handle
        .send(pid.clone(), MessageHandle::new(Hello("hello-2".to_string())))
        .await;
    }
    Ok(())
  }

  async fn receive(&self, _: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
    println!("TopActor::receive: msg = {:?}", message_handle);
    Ok(())
  }
}

pub async fn create_my_actor(ctx: ContextHandle) -> ActorHandle {
  ActorHandle::new(TopActor {})
}

pub async fn create_child_actor(ctx: ContextHandle) -> ActorHandle {
  ActorHandle::new(ChildActor {})
}

#[tokio::main]
async fn main() {
  env_logger::init();
  let system = ActorSystem::new(&[]).await;
  let mut root = system.get_root_context().await;

  let props = Props::from_producer_func_with_opts(
    ProducerFunc::new(create_my_actor),
    vec![Props::with_mailbox(unbounded_mpsc_mailbox_creator())],
  )
  .await;

  let pid = root.spawn(props).await;
  for _ in 1..10 {
    root
      .send(pid.clone(), MessageHandle::new(Hello("hello-1".to_string())))
      .await;
  }

  sleep(std::time::Duration::from_secs(3));
}
