use std::env;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart, StopperPart};
use crate::actor::core::props::Props;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::message::ResponseHandle;
use nexus_message_derive_rs::Message;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn example() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let props = Props::from_async_actor_receiver(move |ctx| async move {
    tracing::debug!("msg = {:?}", ctx.get_message_handle_opt().await.unwrap());
    Ok(())
  })
  .await;

  let pid = root_context.spawn(props).await;
  root_context
    .send(pid.clone(), MessageHandle::new("Hello World".to_string()))
    .await;
  sleep(Duration::from_secs(1)).await;

  root_context.stop_future(&pid).await.result().await.unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Request(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Reply(pub String);

#[tokio::test]
async fn example_synchronous() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let async_barrier = AsyncBarrier::new(2);
  let cloned_async_barrier = async_barrier.clone();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let callee_props = Props::from_async_actor_receiver(move |ctx| async move {
    let msg = ctx.get_message_handle_opt().await.expect("message not found");
    tracing::debug!("callee msg = {:?}", msg);
    if let Some(msg) = msg.to_typed::<Request>() {
      tracing::debug!("{:?}", msg);
      ctx.respond(ResponseHandle::new(Reply("PONG".to_string()))).await
    }
    Ok(())
  })
  .await;
  let callee_pid = root_context.spawn(callee_props).await;
  let cloned_callee_pid = callee_pid.clone();

  let caller_props = Props::from_async_actor_receiver(move |mut ctx| {
    let cloned_async_barrier = cloned_async_barrier.clone();
    let cloned_callee_pid = cloned_callee_pid.clone();
    async move {
      let msg = ctx.get_message_handle_opt().await.expect("message not found");
      tracing::debug!("caller msg = {:?}", msg);
      if let Some(AutoReceiveMessage::PreStart) = msg.to_typed::<AutoReceiveMessage>() {
        ctx
          .request(cloned_callee_pid, MessageHandle::new(Request("PING".to_string())))
          .await;
      }
      if let Some(msg) = msg.to_typed::<Reply>() {
        tracing::debug!("{:?}", msg);
        cloned_async_barrier.wait().await;
      }
      Ok(())
    }
  })
  .await;
  let caller_pid = root_context.spawn(caller_props).await;

  async_barrier.wait().await;
  root_context.stop_future(&callee_pid).await.result().await.unwrap();
  root_context.stop_future(&caller_pid).await.result().await.unwrap();
}
