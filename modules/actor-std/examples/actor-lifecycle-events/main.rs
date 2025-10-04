use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, Props};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_message_derive_rs::Message;
use nexus_utils_std_rs::concurrent::WaitGroup;
use std::env;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Hello {
  who: String,
}

#[derive(Debug)]
struct HelloActor {
  error: bool,
  wait_group: WaitGroup,
}

impl HelloActor {
  fn new(wait_group: WaitGroup) -> Self {
    Self {
      error: true,
      wait_group,
    }
  }
}

#[async_trait]
impl Actor for HelloActor {
  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message_handle = ctx.get_message_handle_opt().await.expect("message not found");
    let hello = message_handle.to_typed::<Hello>().unwrap();
    tracing::info!("Hello, {}!", hello.who);
    if self.error {
      Err(ActorError::ReceiveError(ErrorReason::new("Ouch".to_string(), 0)))
    } else {
      Ok(())
    }
  }

  async fn post_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("Started, initialize actor here");
    Ok(())
  }

  async fn pre_restart(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("Restarting, actor is about restart");
    Ok(())
  }

  async fn pre_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("Stopping, actor is about shut down");
    Ok(())
  }

  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::info!("Stopped, actor and its children are stopped");
    self.wait_group.done();
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  env::set_var("RUST_LOG", "actor_lifecycle_events=info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let actor_system = ActorSystem::new().await.expect("Failed to create actor system");
  let mut root_context = actor_system.get_root_context().await;
  let wait_group = WaitGroup::with_count(1);
  let cloned_wait_group = wait_group.clone();
  let props = Props::from_sync_actor_producer(move |_| HelloActor::new(cloned_wait_group.clone())).await;
  let pid = root_context.spawn(props).await;
  root_context
    .send(
      pid.clone(),
      MessageHandle::new(Hello {
        who: "world".to_string(),
      }),
    )
    .await;

  sleep(std::time::Duration::from_secs(1)).await;
  root_context.stop(&pid).await;
  wait_group.wait().await;
}
