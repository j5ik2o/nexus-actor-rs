use async_trait::async_trait;
use nexus_acto_rs::actor::actor::actor::Actor;
use nexus_acto_rs::actor::actor::actor_error::ActorError;
use nexus_acto_rs::actor::actor::actor_inner_error::ActorInnerError;
use nexus_acto_rs::actor::actor::props::Props;
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::context::context_handle::ContextHandle;
use nexus_acto_rs::actor::context::{MessagePart, SenderPart, SpawnerPart};
use nexus_acto_rs::actor::message::message::Message;
use nexus_acto_rs::actor::message::message_handle::MessageHandle;
use nexus_acto_rs::actor::supervisor::directive::Directive;
use nexus_acto_rs::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
use nexus_acto_rs::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
use nexus_acto_rs::actor::util::async_barrier::AsyncBarrier;
use std::any::Any;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Debug)]
struct Parent;

impl Parent {
  fn new() -> Self {
    Self
  }
}

#[async_trait]
impl Actor for Parent {
  async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    let message_handle = context_handle.get_message_handle().await;
    let msg = message_handle.to_typed::<Hello>().unwrap();
    let props = Props::from_actor_producer(|_| async { Child::new() }).await;
    let child = context_handle.spawn(props).await;
    context_handle.send(child, MessageHandle::new(msg)).await;
    Ok(())
  }
}

#[derive(Debug)]
struct Child;

impl Child {
  fn new() -> Self {
    Self
  }
}

#[async_trait]
impl Actor for Child {
  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message_handle = ctx.get_message_handle().await;
    let msg = message_handle.to_typed::<Hello>().unwrap();
    println!("Hello, {}", msg.who);
    msg.async_barrier.wait().await;
    Err(ActorError::ReceiveError(ActorInnerError::new("Ouch".to_string())))
  }
}

#[derive(Debug, Clone)]
struct Hello {
  who: String,
  async_barrier: AsyncBarrier,
}

impl Hello {
  fn new(who: String, async_barrier: AsyncBarrier) -> Self {
    Self { who, async_barrier }
  }
}

impl Message for Hello {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.eq_message(self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[tokio::main]
async fn main() {
  let _ = env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await;
  let decider = |_| async {
    println!("occurred error");
    Directive::Stop
  };
  let supervisor = OneForOneStrategy::new(10, Duration::from_millis(1000)).with_decider(decider);
  let mut root_context = system.get_root_context().await;
  let props = Props::from_actor_producer_with_opts(
    |_| async { Parent::new() },
    [Props::with_supervisor_strategy(SupervisorStrategyHandle::new(
      supervisor,
    ))],
  )
  .await;
  let pid = root_context.spawn(props).await;
  let async_barrier = AsyncBarrier::new(2);
  root_context
    .send(
      pid,
      MessageHandle::new(Hello::new("Roger".to_string(), async_barrier.clone())),
    )
    .await;
  async_barrier.wait().await;
  sleep(Duration::from_secs(2)).await;
}
