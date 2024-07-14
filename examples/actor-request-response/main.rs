use nexus_acto_rs::actor::actor::props::Props;
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart};
use nexus_acto_rs::actor::message::message::Message;
use nexus_acto_rs::actor::message::message_handle::MessageHandle;
use nexus_acto_rs::actor::message::message_or_envelope::unwrap_envelope_message;
use nexus_acto_rs::actor::message::response::ResponseHandle;
use std::any::Any;
use std::time::Duration;

#[derive(Debug, Clone)]
struct Hello {
  who: String,
}

impl Message for Hello {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let other_msg = other.as_any().downcast_ref::<Hello>();
    match other_msg {
      Some(other_msg) => self.who == other_msg.who,
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[tokio::main]
async fn main() {
  let system = ActorSystem::new().await;
  let mut root_context = system.get_root_context().await;
  let props = Props::from_actor_receiver(|ctx| async move {
    if let Some(msg) = unwrap_envelope_message(ctx.get_message_handle().await).to_typed::<Hello>() {
      ctx.respond(ResponseHandle::new(format!("Hello, {}!", msg.who))).await;
    }
    Ok(())
  })
  .await;
  let pid = root_context.spawn(props).await;
  let msg = MessageHandle::new(Hello {
    who: "world".to_string(),
  });
  let future = root_context.request_future(pid, msg, Duration::from_secs(1)).await;
  let result = future.result().await;
  println!("{:?}", result.unwrap());
}
