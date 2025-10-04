use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::core::{Props, TypedProps};
use nexus_actor_std_rs::actor::dispatch::{
  unbounded_mailbox_creator_with_opts, MailboxMiddleware, MailboxMiddlewareHandle,
};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::actor::typed_context::{TypedSenderPart, TypedSpawnerPart};
use std::env;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Debug)]
struct MailboxLogger {}

impl MailboxLogger {
  pub fn new() -> Self {
    MailboxLogger {}
  }
}

#[async_trait]
impl MailboxMiddleware for MailboxLogger {
  async fn mailbox_started(&mut self) {
    tracing::info!("Mailbox started");
  }

  async fn message_posted(&mut self, message_handle: &MessageHandle) {
    tracing::info!("Message posted: {:?}", message_handle);
  }

  async fn message_received(&mut self, message_handle: &MessageHandle) {
    tracing::info!("Message received: {:?}", message_handle);
  }

  async fn mailbox_empty(&mut self) {
    tracing::info!("Mailbox empty");
  }
}

#[tokio::main]
async fn main() {
  unsafe {
    env::set_var("RUST_LOG", "actor_mailbox_middleware=info");
  }
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await.to_typed();
  let props = TypedProps::from_async_actor_receiver_with_opts(
    move |_| async move { Ok(()) },
    [Props::with_mailbox_producer(unbounded_mailbox_creator_with_opts([
      MailboxMiddlewareHandle::new(MailboxLogger::new()),
    ]))],
  )
  .await;

  let pid = root_context.spawn(props).await;
  root_context.send(pid.clone(), "Hello".to_string()).await;
  sleep(std::time::Duration::from_secs(1)).await;
  root_context.send(pid, "Hello".to_string()).await;
  sleep(std::time::Duration::from_secs(5)).await;
}
