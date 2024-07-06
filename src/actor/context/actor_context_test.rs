#[cfg(test)]
mod tests {
  use crate::actor::actor::actor_receiver::ActorReceiver;
  use crate::actor::actor::continuer::Continuer;
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, InfoPart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::message_or_envelope::MessageEnvelope;
  use crate::actor::message::response::ResponseHandle;
  use std::env;
  use std::time::Duration;
  use tracing_subscriber::EnvFilter;

  #[tokio::test]
  async fn test_actor_continue_future_in_actor() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;
    let pid = root_context
      .spawn(
        Props::from_actor_receiver(ActorReceiver::new(move |ctx| async move {
          let msg = ctx.get_message().await;
          if let Some(me) = msg.as_any().downcast_ref::<MessageEnvelope>() {
            let self_pid = ctx.get_self().await;
            let msg = me.get_message().as_any().downcast_ref::<String>().unwrap().clone();
            if msg == "request" {
              ctx.respond(ResponseHandle::new("done".to_string())).await;
            }
            if msg == "start" {
              let future = ctx
                .request_future(
                  self_pid,
                  MessageHandle::new("request".to_string()),
                  Duration::from_secs(5),
                )
                .await;
              let cloned_ctx = ctx.clone();
              ctx
                .reenter_after(
                  future,
                  Continuer::new(move |msg, _| {
                    let cloned_ctx = cloned_ctx.clone();
                    let cloned_msg = msg.clone().unwrap().as_any().downcast_ref::<String>().unwrap().clone();
                    async move {
                      cloned_ctx.respond(ResponseHandle::new(cloned_msg)).await;
                    }
                  }),
                )
                .await
            }
          }
          Ok(())
        }))
        .await,
      )
      .await;
    let res = root_context
      .request_future(pid, MessageHandle::new("start".to_string()), Duration::from_secs(10))
      .await
      .result()
      .await
      .unwrap();
    tracing::debug!("res = {:?}", res);
    let response = res.as_any().downcast_ref::<String>().unwrap().clone();
    assert_eq!(response, "done".to_string());
  }
}
