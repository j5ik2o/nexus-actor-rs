use async_trait::async_trait;
use nexus_acto_rs::actor::actor::{Props, TypedProps};
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::dispatch::{unbounded_mailbox_creator_with_opts, MailboxMiddleware, MailboxMiddlewareHandle};
use nexus_acto_rs::actor::message::MessageHandle;
use nexus_acto_rs::actor::typed_context::{TypedMessagePart, TypedSenderPart, TypedSpawnerPart};
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
  async fn mailbox_started(&self) {
    tracing::info!("Mailbox started");
  }

  async fn message_posted(&self, message_handle: MessageHandle) {
    tracing::info!("Message posted: {:?}", message_handle);
  }

  async fn message_received(&self, message_handle: MessageHandle) {
    tracing::info!("Message received: {:?}", message_handle);
  }

  async fn mailbox_empty(&self) {
    tracing::info!("Mailbox empty");
  }
}

#[tokio::main]
async fn main() {
  let _ = env::set_var("RUST_LOG", "info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await;
  let mut root = system.get_typed_root_context().await;
  let props = TypedProps::from_actor_receiver_with_opts(
    move |ctx| async move { Ok(()) },
    [Props::with_mailbox_producer(unbounded_mailbox_creator_with_opts([
      MailboxMiddlewareHandle::new(MailboxLogger::new()),
    ]))],
  )
  .await;

  let pid = root.spawn(props).await;
  root.send(pid.clone(), "Hello".to_string()).await;
  sleep(std::time::Duration::from_secs(1)).await;
  root.send(pid, "Hello".to_string()).await;
  sleep(std::time::Duration::from_secs(5)).await;
}
