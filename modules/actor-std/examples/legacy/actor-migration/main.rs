use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use std::any::Any;
use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static MESSAGE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
struct StartPing {
  target: ExtendedPid,
}

#[derive(Debug, Clone)]
struct Ping {
  from: String,
}

#[derive(Debug, Clone)]
struct Pong {
  from: String,
  count: usize,
}

impl Message for StartPing {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<StartPing>()
      .map(|rhs| rhs.target == self.target)
      .unwrap_or(false)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "StartPing".into()
  }
}

impl Message for Ping {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<Ping>()
      .map(|rhs| rhs.from == self.from)
      .unwrap_or(false)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "Ping".into()
  }
}

impl Message for Pong {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<Pong>()
      .map(|rhs| rhs.from == self.from && rhs.count == self.count)
      .unwrap_or(false)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "Pong".into()
  }
}

#[derive(Debug)]
struct PingActor {
  name: String,
  pong_count: usize,
}

impl PingActor {
  fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      pong_count: 0,
    }
  }
}

#[async_trait]
impl Actor for PingActor {
  async fn pre_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("{} started", self.name);
    Ok(())
  }

  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if let Some(start) = message.to_typed::<StartPing>() {
      println!("{} initiating ping-pong with {}", self.name, start.target);
      ctx
        .send(
          start.target,
          MessageHandle::new(Ping {
            from: self.name.clone(),
          }),
        )
        .await;
      return Ok(());
    }

    if let Some(pong) = message.to_typed::<Pong>() {
      self.pong_count += 1;
      println!("{} received Pong #{} from {}", self.name, pong.count, pong.from);

      if pong.count < 5 {
        if let Some(sender) = ctx.get_sender().await {
          ctx
            .send(
              sender.clone(),
              MessageHandle::new(Ping {
                from: self.name.clone(),
              }),
            )
            .await;
        }
      } else {
        println!("{} finished ping-pong after {} exchanges", self.name, self.pong_count);
      }
      return Ok(());
    }

    Ok(())
  }

  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("{} stopped", self.name);
    Ok(())
  }
}

#[derive(Debug)]
struct PongActor {
  name: String,
  ping_count: usize,
}

impl PongActor {
  fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      ping_count: 0,
    }
  }
}

#[async_trait]
impl Actor for PongActor {
  async fn pre_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("{} started", self.name);
    Ok(())
  }

  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if let Some(ping) = message.to_typed::<Ping>() {
      self.ping_count += 1;
      let total = MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
      println!(
        "{} received Ping from {} (total messages: {})",
        self.name, ping.from, total
      );

      if let Some(sender) = ctx.get_sender().await {
        ctx
          .send(
            sender.clone(),
            MessageHandle::new(Pong {
              from: self.name.clone(),
              count: self.ping_count,
            }),
          )
          .await;
      }
    }

    Ok(())
  }

  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("{} stopped after {} pings", self.name, self.ping_count);
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  println!("=== Actor Migration Example (Actor トレイト版) ===\n");

  let system = ActorSystem::new().await.expect("actor system");
  let mut root = system.get_root_context().await;

  let ping_props = Props::from_async_actor_producer(|_| async { PingActor::new("PingActor") }).await;
  let pong_props = Props::from_async_actor_producer(|_| async { PongActor::new("PongActor") }).await;

  let ping_pid = root.spawn(ping_props).await;
  let pong_pid = root.spawn(pong_props).await;

  root
    .send(
      ping_pid.clone(),
      MessageHandle::new(StartPing {
        target: pong_pid.clone(),
      }),
    )
    .await;

  tokio::time::sleep(Duration::from_secs(1)).await;

  root.stop(&ping_pid).await;
  root.stop(&pong_pid).await;

  tokio::time::sleep(Duration::from_millis(200)).await;

  println!("\n=== Example completed! ===");
}
