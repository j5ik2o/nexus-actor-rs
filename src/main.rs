use std::sync::Arc;
use std::thread::sleep;

use async_trait::async_trait;

use nexus_rs::actor::actor::{Actor, ActorError, ActorHandle};
use nexus_rs::actor::actor_system::ActorSystem;
use nexus_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_rs::actor::message::{Message, MessageHandle, ProducerFunc};
use nexus_rs::actor::messages::SystemMessage;
use nexus_rs::actor::props::Props;
use nexus_rs::actor::unbounded::unbounded_mpsc_mailbox_creator;

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
  async fn receive(&self, ctx: ContextHandle) -> Result<(), ActorError> {
    let msg = ctx.get_message().await;
    println!("child_actor: msg = {:?}", msg);
    Ok(())
  }
}

#[derive(Debug)]
struct MyActor {}

#[async_trait]
impl Actor for MyActor {
  async fn receive(&self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let msg = ctx.get_message().await.unwrap();
    println!("my_actor: msg = {:?}", msg);

    if let Some(sm) = msg.as_any().downcast_ref::<SystemMessage>() {
      println!("string = {:?}", sm);
      let props = Props::from_producer_func(ProducerFunc::new(create_child_actor)).await;

      let pid = ctx.spawn(props).await;
      println!("child = {}", pid);
      for _ in 1..10 {
        ctx
          .send(pid.clone(), MessageHandle::new(Hello("hello-2".to_string())))
          .await;
      }
    }
    Ok(())
  }
}

pub async fn create_my_actor(ctx: ContextHandle) -> ActorHandle {
  let actor = MyActor {};
  ActorHandle::new(Arc::new(actor))
}

pub async fn create_child_actor(ctx: ContextHandle) -> ActorHandle {
  let actor = ChildActor {};
  ActorHandle::new(Arc::new(actor))
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
