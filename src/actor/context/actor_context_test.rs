#[cfg(test)]
mod tests {
  use crate::actor::actor::actor_receiver::ActorReceiver;
  use crate::actor::actor::continuer::Continuer;
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, InfoPart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::message::Message;
  use std::env;

  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::message_or_envelope::MessageEnvelope;
  use crate::actor::message::response::ResponseHandle;
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
          if let Some(msg) = ctx.get_message().await {
            tracing::debug!("msg = {:?}", msg);
            tracing::debug!("ctx.sender = {:?}", ctx.get_sender().await);
            if let Some(me) = msg.as_any().downcast_ref::<MessageEnvelope>() {
              let msg = me.get_message().as_any().downcast_ref::<String>().unwrap().clone();
              if msg == "request" {
                ctx.respond(ResponseHandle::new("done".to_string())).await;
              }
              if msg == "start" {
                let f = ctx
                  .request_future(
                    ctx.get_self().await.unwrap(),
                    MessageHandle::new("request".to_string()),
                    &std::time::Duration::from_secs(5),
                  )
                  .await;
                let cloned_ctx = ctx.clone();
                let cloned_msg = msg.clone();
                tracing::debug!("cloned_ctx.sender = {:?}", cloned_ctx.get_sender().await);
                ctx
                  .reenter_after(
                    f,
                    Continuer::new(move |msg, err| {
                      let cloned_ctx = cloned_ctx.clone();
                      let cloned_mh = msg.clone().unwrap();
                      let cloned_msg = cloned_mh.as_any().downcast_ref::<String>().unwrap().clone();
                      tracing::debug!("msg = {:?}", cloned_msg);
                      async move {
                        cloned_ctx.respond(ResponseHandle::new(cloned_msg)).await;
                      }
                    }),
                  )
                  .await
              }
            }
          }
          Ok(())
        }))
        .await,
      )
      .await;
    let res = root_context
      .request_future(
        pid,
        MessageHandle::new("start".to_string()),
        &std::time::Duration::from_secs(10),
      )
      .await
      .result()
      .await
      .unwrap();
    tracing::debug!("res = {:?}", res);
    let response = res.as_any().downcast_ref::<String>().unwrap().clone();
    assert_eq!(response, "done".to_string());
  }
}
