use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, Props};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_message_derive_rs::Message as MessageDerive;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, MessageDerive)]
struct CreateChild {
  name: String,
}

#[derive(Debug, Clone, PartialEq, MessageDerive)]
struct ChildMessage;

#[derive(Debug)]
struct ChildActor {
  name: String,
  counter: Arc<AtomicUsize>,
}

#[async_trait]
impl Actor for ChildActor {
  async fn pre_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("[Child {}] started", self.name);
    Ok(())
  }

  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if message.is_typed::<ChildMessage>() {
      let current = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
      println!("[Child {}] processed message #{}", self.name, current);
    }

    Ok(())
  }
}

#[derive(Debug)]
struct ParentActor {
  child_counter: Arc<AtomicUsize>,
}

#[async_trait]
impl Actor for ParentActor {
  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if let Some(command) = message.to_typed::<CreateChild>() {
      let child_counter = self.child_counter.clone();
      let child_name = command.name.clone();
      let props = Props::from_async_actor_producer(move |_| {
        let counter = child_counter.clone();
        let name = child_name.clone();
        async move { ChildActor { name, counter } }
      })
      .await;

      let mut ctx_clone = ctx.clone();
      let child_pid = ctx_clone
        .spawn_named(props, &command.name)
        .await
        .map_err(|err| ActorError::ReceiveError(ErrorReason::new(err.to_string(), 0)))?;

      ctx.send(child_pid, MessageHandle::new(ChildMessage)).await;
    }

    Ok(())
  }
}

#[tokio::main]
async fn main() {
  println!("=== Actor Props Example (BaseActor 移行後) ===\n");

  let system = ActorSystem::new().await.expect("actor system");
  let mut root = system.get_root_context().await;

  let child_counter = Arc::new(AtomicUsize::new(0));
  let counter_clone = child_counter.clone();
  let parent_props = Props::from_async_actor_producer(move |_| {
    let counter = counter_clone.clone();
    async move { ParentActor { child_counter: counter } }
  })
  .await;

  let parent_pid = root.spawn(parent_props).await;

  for name in ["child-props-a", "child-props-b"] {
    root
      .send(
        parent_pid.clone(),
        MessageHandle::new(CreateChild { name: name.to_string() }),
      )
      .await;
  }

  tokio::time::sleep(Duration::from_millis(200)).await;
  println!(
    "\nTotal child messages processed: {}",
    child_counter.load(Ordering::SeqCst)
  );
  println!("\n=== Example completed! ===");
}
