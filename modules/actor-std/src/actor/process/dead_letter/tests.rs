#[cfg(test)]
mod test {
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{ContextHandle, SenderPart, SpawnerPart, StopperPart};
  use crate::actor::core::{Actor, ActorError, Props};
  use crate::actor::message::MessageHandle;
  use crate::actor::message::SystemMessage;
  use crate::actor::process::dead_letter_process::DeadLetterEvent;
  use crate::actor::process::future::ActorFutureProcess;
  use async_trait::async_trait;
  use std::env;
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::sync::Mutex;
  use tracing_subscriber::EnvFilter;

  #[derive(Debug, Clone)]
  pub struct BlackHoleActor;

  #[async_trait]
  impl Actor for BlackHoleActor {
    async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_dead_letter_after_stop() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let mut root_context = system.get_root_context().await;

    let a = root_context
      .spawn(Props::from_async_actor_producer(|_| async { BlackHoleActor }).await)
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

  #[tokio::test]
  async fn test_dead_letter_watch_responds_with_terminate() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let mut root_context = system.get_root_context().await;
    let pid = root_context
      .spawn(Props::from_async_actor_producer(|_| async { BlackHoleActor }).await)
      .await;
    let _ = root_context.stop_future(&pid).await.result().await.unwrap();
    let f = ActorFutureProcess::new(system.clone(), Duration::from_secs(5)).await;

    let watcher = f.get_pid().await.clone();
    pid
      .send_system_message(
        system.clone(),
        MessageHandle::new(SystemMessage::watch(watcher.to_core())),
      )
      .await;

    f.result().await.unwrap();
  }
}
