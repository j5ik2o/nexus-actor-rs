#[cfg(test)]
mod tests {
  use crate::actor::actor::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::{MessageBatch, MessageHandle};
  use crate::actor::util::WaitGroup;
  use std::env;
  use tracing_subscriber::EnvFilter;

  #[tokio::test]
  async fn test_actor_receives_each_message_in_amessage_batch() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let seen_messages_wg = WaitGroup::new();
    let cloned_seen_messages_wg = seen_messages_wg.clone();
    seen_messages_wg.add(1).await;

    let system = ActorSystem::new().await.unwrap();

    let props = Props::from_async_actor_receiver(move |ctx| {
      let cloned_seen_messages_wg = cloned_seen_messages_wg.clone();
      async move {
        let message = ctx.get_message_handle().await;
        tracing::debug!("Received message: {:?}", message);
        if let Some(_) = message.to_typed::<MessageBatch>() {
          cloned_seen_messages_wg.done().await;
        }
        Ok(())
      }
    })
    .await;

    let mut root_context = system.get_root_context().await;
    let pid = root_context.spawn(props).await;

    let values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
      .into_iter()
      .map(|v| MessageHandle::new(v))
      .collect::<Vec<_>>();
    let message_batch = MessageHandle::new(MessageBatch::new(values));

    root_context.send(pid, message_batch).await;

    seen_messages_wg.wait().await;
  }
}
