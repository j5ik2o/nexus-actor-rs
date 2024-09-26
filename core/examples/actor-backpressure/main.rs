use async_trait::async_trait;
use nexus_actor_core_rs::actor::actor::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::dispatch::{
  unbounded_mailbox_creator_with_opts, MailboxMiddleware, MailboxMiddlewareHandle,
};
use nexus_actor_core_rs::actor::message::Message;
use nexus_actor_core_rs::actor::message::MessageHandle;
use nexus_actor_core_rs::actor::util::WaitGroup;
use nexus_actor_message_derive_rs::Message;
use std::env;
use std::fmt::Debug;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Message)]
struct RequestMoreWork {
  items: u32,
}

impl RequestMoreWork {
  fn new(items: u32) -> Self {
    Self { items }
  }
}

#[derive(Debug, Clone)]
struct RequestWorkBehavior {
  tokens: Arc<AtomicU64>,
  producer: ExtendedPid,
  actor_system: ActorSystem,
}

impl RequestWorkBehavior {
  fn new(actor_system: ActorSystem, tokens: u64, producer: ExtendedPid) -> Self {
    Self {
      tokens: Arc::new(AtomicU64::new(tokens)),
      producer,
      actor_system,
    }
  }

  async fn request_more(&mut self) {
    self.tokens.store(50, std::sync::atomic::Ordering::Relaxed);
    self
      .actor_system
      .get_root_context()
      .await
      .send(self.producer.clone(), MessageHandle::new(RequestMoreWork::new(50)))
      .await;
  }
}

#[async_trait]
impl MailboxMiddleware for RequestWorkBehavior {
  async fn mailbox_started(&mut self) {}

  async fn message_posted(&mut self, _message_handle: MessageHandle) {}

  async fn message_received(&mut self, _message_handle: MessageHandle) {
    let token_count = self.tokens.load(std::sync::atomic::Ordering::Relaxed);
    if token_count > 0 {
      self.tokens.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
    if token_count == 0 {
      self.request_more().await;
    }
  }

  async fn mailbox_empty(&mut self) {}
}

#[derive(Debug, Clone)]
struct Producer {
  requested_task: u32,
  produced_tasks: u32,
  worker: Option<ExtendedPid>,
  wait_group: WaitGroup,
}

impl Producer {
  fn new(wait_group: WaitGroup) -> Self {
    Self {
      requested_task: 0,
      produced_tasks: 0,
      worker: None,
      wait_group,
    }
  }
}

#[async_trait]
impl Actor for Producer {
  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    if let Some(request_more_work) = ctx.get_message_handle().await.to_typed::<RequestMoreWork>() {
      self.requested_task += request_more_work.items;
      self.produced_tasks = 0;
      ctx.send(ctx.get_self().await, MessageHandle::new(Produce)).await;
    }
    if ctx.get_message_handle().await.to_typed::<Produce>().is_some() {
      self.produced_tasks += 1;
      ctx
        .send(
          self.worker.clone().expect("Not found"),
          MessageHandle::new(Task::new(self.produced_tasks)),
        )
        .await;
      tracing::info!("Producer: produced a task: {:?}", self.produced_tasks);
      if self.requested_task > 0 {
        self.requested_task -= 1;
        ctx.send(ctx.get_self().await, MessageHandle::new(Produce)).await;
      }
    }
    Ok(())
  }

  async fn post_start(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let mb = unbounded_mailbox_creator_with_opts([MailboxMiddlewareHandle::new(RequestWorkBehavior::new(
      ctx.get_actor_system().await,
      0,
      ctx.get_self().await,
    ))]);
    let worker_props = Props::from_sync_actor_producer_with_opts(
      {
        let cloned_wait_group = self.wait_group.clone();
        move |_| Consumer::new(cloned_wait_group.clone())
      },
      [Props::with_mailbox_producer(mb)],
    )
    .await;
    self.worker = Some(ctx.spawn(worker_props).await);
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
struct Produce;

#[derive(Debug, Clone)]
struct Consumer {
  wait_group: WaitGroup,
}

impl Consumer {
  fn new(wait_group: WaitGroup) -> Self {
    Self { wait_group }
  }
}

#[async_trait]
impl Actor for Consumer {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    if let Some(task) = context_handle.get_message_handle().await.to_typed::<Task>() {
      tracing::info!("Consumer: received task: {:?}", task);
      sleep(std::time::Duration::from_millis(100)).await;
      self.wait_group.done().await;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
struct Task {
  id: u32,
}

impl Task {
  fn new(id: u32) -> Self {
    Self { id }
  }
}

#[tokio::main]
async fn main() {
  let _ = env::set_var("RUST_LOG", "actor_backpressure=info");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await.expect("Failed to create an actor system");

  let wait_group = WaitGroup::with_count(100);
  let props = Props::from_sync_actor_producer({
    let cloned_wait_group = wait_group.clone();
    move |_| Producer::new(cloned_wait_group.clone())
  })
  .await;

  tokio::spawn(async move {
    sleep(std::time::Duration::from_secs(1)).await;
    system.get_root_context().await.spawn(props).await;
  });

  wait_group.wait().await;
}
