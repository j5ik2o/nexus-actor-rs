use std::env;
use std::time::Duration;

use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

use crate::actor::actor::props::Props;
use crate::actor::actor::receive_func::ReceiveFunc;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{MessagePart, SenderPart, SpawnerPart, StopperPart};
use crate::actor::message::message_handle::MessageHandle;

#[tokio::test]
async fn example() {
  let _ = env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await;
  let mut root_context = system.get_root_context().await;

  let props = Props::from_receive_func(ReceiveFunc::new(move |ctx| async move {
    tracing::debug!("msg = {:?}", ctx.get_message().await.unwrap());
    Ok(())
  }))
  .await;

  let pid = root_context.spawn(props).await;
  root_context
    .send(pid.clone(), MessageHandle::new("Hello World".to_string()))
    .await;
  sleep(Duration::from_secs(1)).await;

  let future = root_context.stop_future(&pid).await;
  future.result().await.unwrap();
}
