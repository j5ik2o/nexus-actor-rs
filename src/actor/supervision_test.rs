use std::any::Any;
use std::env;
use std::sync::Arc;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::props::Props;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor::{Actor, ActorError, ActorHandle, ActorInnerError};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ContextHandle, SenderPart, SpawnerPart};
use crate::actor::message::{Message, MessageHandle, ProducerFunc};
use crate::actor::supervisor::supervisor_strategy::{SupervisorHandle, SupervisorStrategy, SupervisorStrategyHandle};
use async_trait::async_trait;
use tokio::sync::Notify;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
struct ActorWithSupervisor {
  notify: Arc<Notify>,
}

#[derive(Debug, Clone)]
struct FailingChildActor;

#[derive(Debug, Clone)]
struct StringMessage(String);

impl Message for StringMessage {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[async_trait]
impl Actor for ActorWithSupervisor {
  async fn post_start(&self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("ActorWithSupervisor::post_start");
    let props = Props::from_producer_func(ProducerFunc::new(|ctx| async { ActorHandle::new(FailingChildActor) })).await;
    let child = ctx.spawn(props).await;
    ctx
      .send(child, MessageHandle::new(StringMessage("fail".to_string())))
      .await;
    Ok(())
  }

  async fn receive(&self, _: ContextHandle, _: MessageHandle) -> Result<(), ActorError> {
    tracing::debug!("ActorWithSupervisor::receive");
    Ok(())
  }

  fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    Some(SupervisorStrategyHandle::new(self.clone()))
  }
}

#[async_trait]
impl SupervisorStrategy for ActorWithSupervisor {
  async fn handle_failure(
    &self,
    _: &ActorSystem,
    _: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  ) {
    tracing::debug!(
      "ActorWithSupervisor::handle_failure: child = {:?}, rs = {:?}, message = {:?}",
      child,
      rs,
      message
    );
    self.notify.notify_one();
  }
}

#[async_trait]
impl Actor for FailingChildActor {
  async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("FailingChildActor::post_start");
    Ok(())
  }

  async fn receive(&self, c: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
    tracing::debug!("FailingChildActor::receive: msg = {:?}", message_handle);
    let msg = message_handle.as_any().downcast_ref::<StringMessage>();
    if let Some(StringMessage(msg)) = msg {
      tracing::debug!("FailingChildActor::receive: msg = {:?}", msg);
      Err(ActorError::ReceiveError(ActorInnerError::new("error")))
    } else {
      Ok(())
    }
  }

  fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[tokio::test]
async fn test_actor_with_own_supervisor_can_handle_failure() {
  let _ = env::set_var("RUST_LOG", "debug");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await;
  let mut root = system.get_root_context().await;
  let notify = Arc::new(Notify::new());
  let cloned_notify = notify.clone();
  let props = Props::from_producer_func(ProducerFunc::new(move |_| {
    let cloned_notify = cloned_notify.clone();
    async move {
      ActorHandle::new(ActorWithSupervisor {
        notify: cloned_notify.clone(),
      })
    }
  }))
  .await;
  let pid = root.spawn(props).await;
  tracing::info!("pid = {:?}", pid);
  notify.notified().await;
}
