#[cfg(test)]
mod tests {
  use std::any::Any;
  use std::env;
  use std::time::Duration;

  use tracing_subscriber::EnvFilter;

  use crate::actor::actor::actor_error::ActorError;
  use crate::actor::actor::actor_inner_error::ActorInnerError;
  use crate::actor::actor::continuer::Continuer;
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::auto_respond::AutoRespond;
  use crate::actor::context::{BasePart, InfoPart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::message_or_envelope::MessageEnvelope;
  use crate::actor::message::response::ResponseHandle;
  use crate::actor::message::touched::Touched;

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
        Props::from_actor_receiver(move |ctx| async move {
          if let Some(me) = ctx.get_message_handle().await.to_typed::<MessageEnvelope>() {
            let self_pid = ctx.get_self().await;
            let msg = me.get_message_handle().to_typed::<String>().unwrap().clone();
            if msg == "request" {
              ctx.respond(ResponseHandle::new("done".to_string())).await;
              Ok(())
            } else if msg == "start" {
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
                    let cloned_msg = msg.clone().unwrap().to_typed::<String>().unwrap().clone();
                    async move {
                      cloned_ctx.respond(ResponseHandle::new(cloned_msg)).await;
                    }
                  }),
                )
                .await;
              Ok(())
            } else {
              Err(ActorError::ReceiveError(ActorInnerError::new(format!(
                "unknown message: msg = {}",
                msg
              ))))
            }
          } else {
            Ok(())
          }
        })
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

    let response = res.to_typed::<String>().unwrap().clone();
    assert_eq!(response, "done".to_string());
  }

  #[derive(Debug, Clone)]
  struct DummyAutoRespond {}
  impl Message for DummyAutoRespond {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().is::<DummyAutoRespond>()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[tokio::test]
  async fn test_actor_context_auto_respond_message() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let actor_receiver = move |_| async move { Ok(()) };
    let pid = root_context
      .spawn(Props::from_actor_receiver(actor_receiver).await)
      .await;

    let result = root_context
      .request_future(
        pid,
        MessageHandle::new(AutoRespond::new(move |_| async move {
          ResponseHandle::new(MessageHandle::new(DummyAutoRespond {}))
        })),
        Duration::from_secs(1),
      )
      .await
      .result()
      .await
      .unwrap();

    assert!(result.as_any().is::<DummyAutoRespond>());
  }

  #[tokio::test]
  async fn test_actor_context_auto_respond_touched_message() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let actor_receiver = move |_| async move { Ok(()) };
    let pid = root_context
      .spawn(Props::from_actor_receiver(actor_receiver).await)
      .await;

    let result = root_context
      .request_future(
        pid.clone(),
        MessageHandle::new(AutoRespond::new(move |ctx| async move {
          ResponseHandle::new(MessageHandle::new(Touched {
            who: Some(ctx.get_self().await.inner_pid.clone()),
          }))
        })),
        Duration::from_secs(1),
      )
      .await
      .result()
      .await
      .unwrap();

    let result2 = result.to_typed::<Touched>();
    assert!(result2.is_some());
    assert_eq!(result2.unwrap().who.unwrap(), pid.inner_pid);
  }
}
