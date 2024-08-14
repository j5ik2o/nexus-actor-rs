use async_trait::async_trait;
use futures::TryStreamExt;
use nexus_acto_rs::actor::actor::Actor;
use nexus_acto_rs::actor::actor::ActorError;
use nexus_acto_rs::actor::actor::Props;
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::context::{ContextHandle, TypedSenderPart};
use nexus_acto_rs::actor::context::{MessagePart, SenderPart, SpawnerPart};
use nexus_acto_rs::actor::dispatch::unbounded_mpsc_mailbox_creator;
use nexus_acto_rs::actor::message::Message;
use nexus_acto_rs::actor::message::MessageHandle;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
struct Hello(pub String);

impl Message for Hello {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let other = other.as_any().downcast_ref::<Hello>();
    match other {
      Some(other) => self.0 == other.0,
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug)]
struct ChildActor;

#[async_trait]
impl Actor for ChildActor {
  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("ChildActor::started");
    Ok(())
  }

  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message_handle = ctx.get_message_handle().await;
    tracing::debug!("ChildActor::receive: msg = {:?}", message_handle);
    Ok(())
  }
}

#[derive(Debug)]
struct TopActor;

#[async_trait]
impl Actor for TopActor {
  async fn post_start(&self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("TopActor::post_start");
    let props = Props::from_actor_producer(create_child_actor).await;
    let pid = context_handle.spawn(props).await;
    for _ in 1..10 {
      context_handle
        .send(pid.clone(), MessageHandle::new(Hello("hello-2".to_string())))
        .await;
    }
    Ok(())
  }

  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message_handle = ctx.get_message_handle().await;
    tracing::debug!("TopActor::receive: msg = {:?}", message_handle);
    Ok(())
  }
}

async fn create_top_actor(_: ContextHandle) -> TopActor {
  TopActor
}

async fn create_child_actor(_: ContextHandle) -> ChildActor {
  ChildActor
}

#[tokio::main]
async fn main() {
  let _ = env::set_var("RUST_LOG", "debug");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await;
  //  let mut root = system.get_root_context().await;
  let mut root = system.get_typed_root_context::<Hello>().await;

  let props = Props::from_actor_producer_with_opts(
    create_top_actor,
    [Props::with_mailbox_producer(unbounded_mpsc_mailbox_creator())],
  )
  .await;

  let pid = root.spawn(props).await;
  for _ in 1..10 {
    root.send(pid.clone(), Hello("hello-1".to_string())).await;
  }

  sleep(Duration::from_secs(3)).await;
}
