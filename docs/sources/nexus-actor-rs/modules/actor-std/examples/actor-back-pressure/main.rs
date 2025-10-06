use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_std_rs::actor::dispatch::{
  unbounded_mailbox_creator_with_opts, MailboxMiddleware, MailboxMiddlewareHandle, MailboxProducer,
};
use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_message_derive_rs::Message;
use nexus_utils_std_rs::concurrent::WaitGroup;
use std::env;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

// Definition of the request message for more tasks
#[derive(Debug, Clone, PartialEq, Message)]
struct RequestMoreTask {
  items: u32,
}

impl RequestMoreTask {
  fn new(items: u32) -> Self {
    Self { items }
  }
}

// Mailbox middleware for backpressure control
#[derive(Debug, Clone)]
struct RequestTaskBehavior {
  tokens: Arc<AtomicU64>,
  producer: ExtendedPid,
  actor_system: ActorSystem,
}

impl RequestTaskBehavior {
  fn new(actor_system: ActorSystem, tokens: u64, producer: ExtendedPid) -> Self {
    Self {
      tokens: Arc::new(AtomicU64::new(tokens)),
      producer,
      actor_system,
    }
  }

  // Replenish tokens and request new tasks
  async fn request_more(&mut self) {
    self.tokens.store(50, Ordering::SeqCst);
    self
      .actor_system
      .get_root_context()
      .await
      .send(self.producer.clone(), MessageHandle::new(RequestMoreTask::new(50)))
      .await;
  }
}

#[async_trait]
impl MailboxMiddleware for RequestTaskBehavior {
  async fn mailbox_started(&mut self) {}

  async fn message_posted(&mut self, _message_handle: &MessageHandle) {}

  // Consume a token when a message is received and request more if needed
  async fn message_received(&mut self, _message_handle: &MessageHandle) {
    let token_count = self.tokens.load(std::sync::atomic::Ordering::SeqCst);
    if token_count > 0 {
      self.tokens.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
    if token_count == 0 {
      self.request_more().await;
    }
  }

  async fn mailbox_empty(&mut self) {}
}

// Producer actor that generates tasks
#[derive(Debug, Clone)]
struct Producer {
  requested_tasks: u32,
  produced_tasks: u32,
  consumer: Option<ExtendedPid>,
  wait_group: WaitGroup,
}

impl Producer {
  fn new(wait_group: WaitGroup) -> Self {
    Self {
      requested_tasks: 0,
      produced_tasks: 0,
      consumer: None,
      wait_group,
    }
  }

  // Generate a mailbox producer with backpressure control
  async fn mailbox_producer(&self, ctx: ContextHandle) -> MailboxProducer {
    unbounded_mailbox_creator_with_opts([MailboxMiddlewareHandle::new(RequestTaskBehavior::new(
      ctx.get_actor_system().await,
      0,
      ctx.get_self().await,
    ))])
  }

  // Generate properties for the consumer actor
  async fn consumer_props(&self, ctx: ContextHandle) -> Props {
    Props::from_sync_actor_producer_with_opts(
      {
        let cloned_wait_group = self.wait_group.clone();
        move |_| Consumer::new(cloned_wait_group.clone())
      },
      [Props::with_mailbox_producer(self.mailbox_producer(ctx.clone()).await)],
    )
    .await
  }
}

#[async_trait]
impl Actor for Producer {
  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    // Handle requests for more tasks
    if let Some(request_more_work) = ctx
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<RequestMoreTask>()
    {
      self.requested_tasks += request_more_work.items;
      self.produced_tasks = 0;
      ctx.send(ctx.get_self().await, MessageHandle::new(Produce)).await;
    }
    if ctx
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<Produce>()
      .is_some()
    {
      self.produced_tasks += 1;
      ctx
        .send(
          self.consumer.clone().expect("Not found"),
          MessageHandle::new(Task::new(self.produced_tasks)),
        )
        .await;
      tracing::info!("Producer: produced a task: {:?}", self.produced_tasks);
      if self.requested_tasks > 0 {
        self.requested_tasks -= 1;
        ctx.send(ctx.get_self().await, MessageHandle::new(Produce)).await;
      }
    }
    Ok(())
  }

  // Create a consumer when the actor starts
  async fn post_start(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    self.consumer = Some(ctx.spawn(self.consumer_props(ctx.clone()).await).await);
    Ok(())
  }
}

// Message to instruct task generation
#[derive(Debug, Clone, PartialEq, Message)]
struct Produce;

// Consumer actor that processes tasks
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
    // Receive and process tasks
    if let Some(task) = context_handle
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<Task>()
    {
      tracing::info!("Consumer: received task: {:?}", task);
      sleep(std::time::Duration::from_millis(100)).await;
      self.wait_group.done();
    }
    Ok(())
  }
}

// Structure representing a task
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
  // Set up logging
  env::set_var("RUST_LOG", "actor_backpressure=info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  // Create the actor system
  let system = ActorSystem::new().await.expect("Failed to create an actor system");

  // Create a WaitGroup to wait for 100 tasks
  let wait_group = WaitGroup::with_count(100);

  // Define properties for the producer actor
  let props = Props::from_sync_actor_producer({
    let cloned_wait_group = wait_group.clone();
    move |_| Producer::new(cloned_wait_group.clone())
  })
  .await;

  // Spawn the producer actor after a 1-second delay
  tokio::spawn(async move {
    sleep(std::time::Duration::from_secs(1)).await;
    system.get_root_context().await.spawn(props).await;
  });

  // Wait for all tasks to complete
  wait_group.wait().await;
}
