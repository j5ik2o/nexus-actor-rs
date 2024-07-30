#[cfg(test)]
mod test {
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{SenderPart, SpawnerPart, StopperPart};
  use crate::actor::dispatch::dead_letter_process::DeadLetterEvent;
  use crate::actor::interaction_test::tests::BlackHoleActor;
  use crate::actor::message::message_handle::MessageHandle;
  use std::env;
  use std::sync::Arc;
  use tokio::sync::Mutex;
  use tracing_subscriber::EnvFilter;

  #[tokio::test]
  async fn test_dead_letter_after_stop() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let a = root_context
      .spawn(Props::from_actor_producer(|_| async { BlackHoleActor }).await)
      .await;
    let cloned_a = a.clone();

    let done = Arc::new(Mutex::new(false));
    let cloned_done = done.clone();

    let sub = system
      .get_event_stream()
      .await
      .subscribe(move |msg| {
        let cloned_a = cloned_a.clone();
        let cloned_done = cloned_done.clone();
        async move {
          if let Some(dead_letter) = msg.to_typed::<DeadLetterEvent>() {
            if dead_letter.pid.unwrap() == cloned_a {
              *cloned_done.lock().await = true;
            }
          }
        }
      })
      .await;

    let _ = root_context.stop_future(&a).await.result().await.unwrap();

    root_context.send(a, MessageHandle::new("hello".to_string())).await;

    system.get_event_stream().await.unsubscribe(sub).await;

    assert!(*done.lock().await);
  }
}
