use std::env;

use async_trait::async_trait;
use tokio::time::sleep;
use nexus_acto_rs::actor::actor::actor::Actor;
use nexus_acto_rs::actor::actor::actor_error::ActorError;
use nexus_acto_rs::actor::actor::actor_handle::ActorHandle;
use nexus_acto_rs::actor::actor::actor_producer::ActorProducer;
use nexus_acto_rs::actor::actor::props::Props;
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::context::context_handle::ContextHandle;
use nexus_acto_rs::actor::context::{SenderPart, SpawnerPart};
use nexus_acto_rs::actor::dispatch::unbounded::unbounded_mpsc_mailbox_creator;
use nexus_acto_rs::actor::message::message::Message;
use nexus_acto_rs::actor::message::message_handle::MessageHandle;
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
struct ChildActor {}

#[async_trait]
impl Actor for ChildActor {
  async fn started(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("ChildActor::started");
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
    tracing::debug!("ChildActor::receive: msg = {:?}", message_handle);
    Ok(())
  }
}

#[derive(Debug)]
struct TopActor {}

#[async_trait]
impl Actor for TopActor {
  async fn started(&self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("TopActor::post_start");
    let props = Props::from_actor_producer(ActorProducer::new(create_child_actor)).await;

    let pid = context_handle.spawn(props).await;
    for _ in 1..10 {
      context_handle
        .send(pid.clone(), MessageHandle::new(Hello("hello-2".to_string())))
        .await;
    }
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
    tracing::debug!("TopActor::receive: msg = {:?}", message_handle);
    Ok(())
  }
}

pub async fn create_top_actor(_: ContextHandle) -> ActorHandle {
  ActorHandle::new(TopActor {})
}

pub async fn create_child_actor(_: ContextHandle) -> ActorHandle {
  ActorHandle::new(ChildActor {})
}

#[tokio::main]
async fn main() {
  let _ = env::set_var("RUST_LOG", "debug");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();
  let system = ActorSystem::new().await;
  let mut root = system.get_root_context().await;

  let props = Props::from_actor_producer_with_opts(
    ActorProducer::new(create_top_actor),
    vec![Props::with_mailbox_producer(unbounded_mpsc_mailbox_creator())],
  )
  .await;

  let pid = root.spawn(props).await;
  for _ in 1..10 {
    root
      .send(pid.clone(), MessageHandle::new(Hello("hello-1".to_string())))
      .await;
  }

  sleep(std::time::Duration::from_secs(3)).await;
}
