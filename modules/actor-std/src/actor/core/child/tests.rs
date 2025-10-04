use crate::actor::actor_system::ActorSystem;
use crate::actor::context::ContextHandle;
use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart, StopperPart};
use crate::actor::core::actor::Actor;
use crate::actor::core::actor_error::ActorError;
use crate::actor::core::error_reason::ErrorReason;
use crate::actor::core::pid::ExtendedPid;
use crate::actor::core::props::Props;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::message::ResponseHandle;
use crate::actor::process::future::ActorFutureProcess;
use crate::generated::actor::Pid;
use async_trait::async_trait;
use nexus_message_derive_rs::Message;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use std::any::Any;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct BlackHoleActor;

#[async_trait]
impl Actor for BlackHoleActor {
  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct CreateChildMessage;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct GetChildCountRequest;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct GetChildCountResponse {
  child_count: usize,
}

#[derive(Debug)]
struct CreateChildActor;

#[async_trait]
impl Actor for CreateChildActor {
  async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    let msg = context_handle
      .get_message_handle_opt()
      .await
      .expect("message not found");
    if msg.to_typed::<CreateChildMessage>().is_some() {
      context_handle
        .spawn(Props::from_async_actor_producer(|_| async { BlackHoleActor }).await)
        .await;
    } else if msg.to_typed::<GetChildCountRequest>().is_some() {
      let reply = GetChildCountResponse {
        child_count: context_handle.get_children().await.len(),
      };
      context_handle.respond(ResponseHandle::new(reply)).await;
    } else {
      return Err(ActorError::of_receive_error(ErrorReason::new("Unknown message", 1)));
    }
    Ok(())
  }
}

#[tokio::test]
async fn test_actor_can_create_children() {
  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;
  let pid = root_context
    .spawn(Props::from_async_actor_producer(|_| async { CreateChildActor }).await)
    .await;
  let expected = 10;
  for _ in 0..expected {
    root_context
      .send(pid.clone(), MessageHandle::new(CreateChildMessage))
      .await;
  }
  let fut = root_context
    .request_future(
      pid.clone(),
      MessageHandle::new(GetChildCountRequest),
      std::time::Duration::from_secs(1),
    )
    .await;
  let response = fut.result().await.unwrap();
  let response = response.to_typed::<GetChildCountResponse>().unwrap();
  assert_eq!(response.child_count, expected);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GetChildCountMessage2 {
  reply_directly: ExtendedPid,
  reply_after_stop: ExtendedPid,
}

impl Message for GetChildCountMessage2 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let other_msg = other.as_any().downcast_ref::<GetChildCountMessage2>();
    match other_msg {
      Some(other_msg) => {
        self.reply_directly == other_msg.reply_directly && self.reply_after_stop == other_msg.reply_after_stop
      }
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

#[derive(Debug, Clone)]
struct CreateChildThenStopActor {
  reply_to: Option<ExtendedPid>,
}

#[async_trait]
impl Actor for CreateChildThenStopActor {
  async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    let msg = context_handle
      .get_message_handle_opt()
      .await
      .expect("message not found");
    tracing::debug!("CreateChildThenStopActor: {:?}", msg);
    if msg.to_typed::<CreateChildMessage>().is_some() {
      context_handle
        .spawn(Props::from_async_actor_producer(|_| async { BlackHoleActor }).await)
        .await;
      Ok(())
    } else if let Some(msg) = msg.to_typed::<GetChildCountMessage2>() {
      context_handle
        .send(msg.reply_directly.clone(), MessageHandle::new(true))
        .await;
      self.reply_to = Some(msg.reply_after_stop.clone());
      Ok(())
    } else {
      Ok(())
    }
  }

  async fn post_stop(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!(
      "post_stop: children.len = {:?}",
      context_handle.get_children().await.len()
    );
    let reply = GetChildCountResponse {
      child_count: context_handle.get_children().await.len(),
    };
    context_handle
      .send(self.reply_to.clone().unwrap(), MessageHandle::new(reply))
      .await;
    Ok(())
  }
}

#[tokio::test]
async fn test_actor_can_stop_children() {
  // env::set_var("RUST_LOG", "debug");
  // let _ = tracing_subscriber::fmt()
  //   .with_env_filter(EnvFilter::from_default_env())
  //   .try_init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;
  let a = root_context
    .spawn(Props::from_async_actor_producer(|_| async { CreateChildThenStopActor { reply_to: None } }).await)
    .await;

  let count = 10;
  for _ in 0..count {
    root_context
      .send(a.clone(), MessageHandle::new(CreateChildMessage))
      .await;
  }

  let future1 = ActorFutureProcess::new(system.clone(), Duration::from_secs(5)).await;
  let future2 = ActorFutureProcess::new(system.clone(), Duration::from_secs(5)).await;

  root_context
    .send(
      a.clone(),
      MessageHandle::new(GetChildCountMessage2 {
        reply_directly: future1.get_pid().await,
        reply_after_stop: future2.get_pid().await,
      }),
    )
    .await;

  let r1 = future1.result().await.unwrap();
  assert!(r1.to_typed::<bool>().unwrap());
  root_context.stop(&a).await;

  let r2 = future2.result().await.unwrap();
  assert_eq!(0, r2.to_typed::<GetChildCountResponse>().unwrap().child_count);
}

#[tokio::test]
async fn test_actor_receives_terminated_from_watched() {
  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let child = root_context
    .spawn(Props::from_async_actor_receiver(|_| async { Ok(()) }).await)
    .await;
  let cloned_child = child.clone();
  let future = ActorFutureProcess::new(system.clone(), Duration::from_secs(5)).await;
  let cloned_future = future.clone();

  let ab = AsyncBarrier::new(2);
  let cloned_ab = ab.clone();

  root_context
    .spawn(
      Props::from_async_actor_receiver(move |mut ctx| {
        let cloned_child = cloned_child.clone();
        let cloned_ab = cloned_ab.clone();
        let cloned_future = cloned_future.clone();
        async move {
          let msg = ctx.get_message_handle_opt().await.expect("message not found");
          if let Some(AutoReceiveMessage::PostStart) = msg.to_typed::<AutoReceiveMessage>() {
            ctx.watch_core(&cloned_child.to_core()).await;
            cloned_ab.wait().await;
          }
          if let Some(AutoReceiveMessage::Terminated(ti)) = msg.to_typed::<AutoReceiveMessage>() {
            let mut ac = ctx.to_actor_context().await.unwrap();
            if let Some(core_pid) = ti.who.as_ref() {
              let pid = Pid::from_core(core_pid.clone());
              if pid == cloned_child.inner_pid && ac.ensure_extras().await.get_watchers().await.is_empty().await {
                ctx.send(cloned_future.get_pid().await, MessageHandle::new(true)).await;
              }
            }
          }
          Ok(())
        }
      })
      .await,
    )
    .await;
  ab.wait().await;
  root_context.stop(&child).await;

  future.result().await.unwrap();
}

#[tokio::test]
async fn test_future_does_timeout() {
  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let pid = root_context
    .spawn(Props::from_async_actor_receiver(|_| async { Ok(()) }).await)
    .await;
  let future = root_context
    .request_future(pid, MessageHandle::new("".to_string()), Duration::from_millis(1))
    .await;
  let result = future.result().await;
  assert!(result.is_err());
}

#[tokio::test]
async fn test_future_does_not_timeout() {
  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;

  let pid = root_context
    .spawn(
      Props::from_async_actor_receiver(|ctx| async move {
        let msg = ctx.get_message_handle_opt().await.expect("message not found");
        if msg.to_typed::<String>().is_some() {
          sleep(Duration::from_millis(50)).await;
          ctx.respond(ResponseHandle::new("foo".to_string())).await;
        }
        Ok(())
      })
      .await,
    )
    .await;
  let future = root_context
    .request_future(pid, MessageHandle::new("".to_string()), Duration::from_secs(2))
    .await;
  let result = future.result().await;
  assert!(result.is_ok());
  let msg_handle = result.unwrap();
  let msg = msg_handle.to_typed::<String>().unwrap();
  assert_eq!("foo", msg);
}
