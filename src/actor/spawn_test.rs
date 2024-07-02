use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokio::sync::Notify;

use crate::actor::actor::{Actor, ActorError, ActorHandle};
use crate::actor::actor::props::Props;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ContextHandle, SpawnerPart};
use crate::actor::message::{MessageHandle, ProducerFunc};

#[derive(Debug, Clone)]
struct MyActor {
  is_started: Arc<AtomicBool>,
  received: Arc<Notify>,
}

#[async_trait]
impl Actor for MyActor {
  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    println!("MyActor started");
    self.is_started.store(true, Ordering::SeqCst);
    self.received.notify_one();
    Ok(())
  }

  async fn receive(&self, _: ContextHandle, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }
}

#[tokio::test]
async fn test_example() {
  let system = ActorSystem::new().await;

  let actor = MyActor {
    is_started: Arc::new(AtomicBool::new(false)),
    received: Arc::new(Notify::new()),
  };

  let producer_func = ProducerFunc::new({
    let actor = actor.clone();
    move |ctx| {
      let actor = actor.clone();
      async move { ActorHandle::new(actor.clone()) }
    }
  });

  let props = Props::from_producer_func(producer_func).await;

  system.get_root_context().await.spawn(props).await;

  actor.received.notified().await;

  assert_eq!(actor.is_started.load(Ordering::SeqCst), true);
}
