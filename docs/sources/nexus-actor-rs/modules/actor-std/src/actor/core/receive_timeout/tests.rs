use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ActorContext, ContextHandle};
use crate::actor::context::{BasePart, MessagePart, SpawnerPart, StopperPart};
use crate::actor::core::actor::Actor;
use crate::actor::core::actor_error::ActorError;
use crate::actor::core::props::Props;
use crate::actor::message::ReceiveTimeout;
use async_trait::async_trait;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use std::env;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
struct SetReceiveTimeoutActor {
  barrier: AsyncBarrier,
}

impl SetReceiveTimeoutActor {
  pub fn new(barrier: AsyncBarrier) -> Self {
    Self { barrier }
  }
}

#[async_trait]
impl Actor for SetReceiveTimeoutActor {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let msg = context_handle
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<ReceiveTimeout>();
    if msg.is_some() {
      tracing::debug!("ReceiveTimeout");
      self.barrier.wait().await;
    }
    Ok(())
  }

  async fn post_start(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    ctx.set_receive_timeout(&Duration::from_millis(100)).await;
    Ok(())
  }
}

#[tokio::test]
async fn test_example_context_set_receive_timeout() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();
  let b = AsyncBarrier::new(2);

  let system = ActorSystem::new().await.unwrap();
  let cloned_b = b.clone();

  let mut root_context = system.get_root_context().await;

  let pid = root_context
    .spawn(
      Props::from_async_actor_producer(move |_| {
        let cloned_b = cloned_b.clone();
        async move { SetReceiveTimeoutActor::new(cloned_b.clone()) }
      })
      .await,
    )
    .await;

  b.wait().await;

  let result = root_context.stop_future(&pid).await;

  result.result().await.unwrap();
}

#[tokio::test]
async fn test_set_receive_timeout_allows_zero_duration() {
  let system = ActorSystem::new().await.unwrap();
  let props = Props::from_async_actor_receiver(|_ctx| async move { Ok(()) }).await;

  let mut ctx = ActorContext::new(system.clone(), props.clone(), None).await;

  ctx.set_receive_timeout(&Duration::from_millis(50)).await;
  assert_eq!(ctx.get_receive_timeout().await, Duration::from_millis(50));

  ctx.set_receive_timeout(&Duration::ZERO).await;
  assert_eq!(ctx.get_receive_timeout().await, Duration::ZERO);

  // values below 1ms should be treated as zero and cancel the timer
  ctx.set_receive_timeout(&Duration::from_micros(500)).await;
  assert_eq!(ctx.get_receive_timeout().await, Duration::ZERO);
}
