use std::env;
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::config::MetricsProvider;
use crate::actor::config_option::ConfigOption;
use crate::actor::context::{ActorContext, BasePart, InfoPart, MessagePart, SenderPart, SpawnerPart};
use crate::actor::core::ActorError;
use crate::actor::core::Continuer;
use crate::actor::core::ErrorReason;
use crate::actor::core::Props;
use crate::actor::message::AutoRespond;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::message::ResponseHandle;
use crate::actor::message::Touched;
use nexus_actor_core_rs::runtime::CoreSpawner as _;
use nexus_actor_core_rs::runtime::CoreTaskFuture;
use nexus_message_derive_rs::Message;
use nexus_utils_std_rs::runtime::TokioCoreSpawner;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use tokio::task::yield_now;
use tokio::time::timeout;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn test_actor_continue_future_in_actor() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let pid = root_context
    .spawn(
      Props::from_async_actor_receiver(move |ctx| async move {
        if let Some(msg) = ctx
          .get_message_handle_opt()
          .await
          .expect("message not found")
          .to_typed::<String>()
        {
          let self_pid = ctx.get_self().await;
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
            Err(ActorError::ReceiveError(ErrorReason::new(
              format!("unknown message: msg = {}", msg),
              0,
            )))
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

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct DummyAutoRespond {}

#[tokio::test]
async fn test_actor_context_auto_respond_message() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let actor_receiver = move |_| async move { Ok(()) };
  let pid = root_context
    .spawn(Props::from_async_actor_receiver(actor_receiver).await)
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
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let actor_receiver = move |_| async move { Ok(()) };
  let pid = root_context
    .spawn(Props::from_async_actor_receiver(actor_receiver).await)
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

#[tokio::test]
async fn test_actor_context_metrics_reentrancy() {
  let system = ActorSystem::new_config_options([ConfigOption::SetMetricsProvider(Arc::new(MetricsProvider::Sdk(
    SdkMeterProvider::default(),
  )))])
  .await
  .expect("actor system with metrics");

  let props = Props::from_async_actor_receiver(|_| async move { Ok(()) }).await;
  let ctx = ActorContext::new(system.clone(), props, None).await;

  let ctx_spawn = ctx.clone();
  let task: CoreTaskFuture = Box::pin(async move {
    loop {
      if ctx_spawn.metrics_sink_or_init().is_some() {
        break;
      }
      yield_now().await;
    }
    let _ = ctx_spawn.get_self_opt().await;
  });
  let join_handle = TokioCoreSpawner::current().spawn(task).expect("spawn metrics task");

  let result = timeout(Duration::from_millis(200), async move {
    loop {
      if ctx.metrics_sink_or_init().is_some() {
        break;
      }
      yield_now().await;
    }
    join_handle.clone().join().await;
  })
  .await;

  assert!(result.is_ok(), "メトリクス同期アクセスがデッドロックせず完了すること");
}
