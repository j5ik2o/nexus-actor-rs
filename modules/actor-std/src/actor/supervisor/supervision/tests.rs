#[cfg(test)]
mod test {
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::ContextHandle;
  use crate::actor::context::ReceiverContextHandle;
  use crate::actor::context::{MessagePart, SenderPart, SpawnerPart};
  use crate::actor::core::Actor;
  use crate::actor::core::ActorError;
  use crate::actor::core::ErrorReason;
  use crate::actor::core::Props;
  use crate::actor::core::ReceiverMiddleware;
  use crate::actor::core::RestartStatistics;
  use crate::actor::message::AutoReceiveMessage;
  use crate::actor::message::Message;
  use crate::actor::message::MessageHandle;
  use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
  use crate::actor::supervisor::supervisor_strategy::{SupervisorHandle, SupervisorStrategy};
  use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
  use async_trait::async_trait;
  use nexus_actor_core_rs::actor::core_types::pid::CorePid;
  use nexus_message_derive_rs::Message;
  use std::any::Any;
  use std::collections::VecDeque;
  use std::env;
  use std::sync::Arc;
  use std::time::Duration;
  use thiserror::Error;
  use tokio::sync::{Mutex, Notify};
  use tokio::time::Instant;
  use tracing_subscriber::EnvFilter;

  #[tokio::test]
  async fn test_actor_with_own_supervisor_can_handle_failure() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let mut root = system.get_root_context().await;
    let notify = Arc::new(Notify::new());
    let cloned_notify = notify.clone();
    let props = Props::from_async_actor_producer(move |_| {
      let cloned_notify = cloned_notify.clone();
      async move {
        ActorWithSupervisor {
          notify: cloned_notify.clone(),
        }
      }
    })
    .await;
    let pid = root.spawn(props).await;
    tracing::info!("pid = {:?}", pid);
    notify.notified().await;
  }

  #[tokio::test]
  async fn test_actor_stops_after_x_restarts() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();
    let observer = Observer::new();
    let system = ActorSystem::new().await.unwrap();
    let mut root_context = system.get_root_context().await;

    let cloned_observer = observer.clone();
    let middles = ReceiverMiddleware::from_async(move |snapshot, next| {
      let cloned_observer = cloned_observer.clone();
      Box::pin(async move {
        tracing::debug!("ReceiverMiddleware: envelope = {:?}", snapshot.message());
        let msg = snapshot.message().get_message_handle();
        tracing::debug!(">>>> msg = {:?}", msg);

        let context_handle = snapshot
          .context()
          .context_handle()
          .cloned()
          .expect("receiver snapshot missing context handle");

        cloned_observer
          .receive(ReceiverContextHandle::new(context_handle), msg.clone())
          .await?;

        next(snapshot).await
      })
    });

    let props = Props::from_async_actor_producer_with_opts(
      |_| async { FailingChildActor },
      [
        Props::with_receiver_middlewares([middles]),
        Props::with_supervisor_strategy(SupervisorStrategyHandle::new(OneForOneStrategy::new(
          10,
          Duration::from_secs(10),
        ))),
      ],
    )
    .await;

    let child = root_context.spawn(props).await;
    let fail = MessageHandle::new(StringMessage("fail".to_string()));
    let d = Duration::from_secs(10);
    let _ = observer
      .expect_message(MessageHandle::new(AutoReceiveMessage::PreStart), d)
      .await;
    let _ = observer
      .expect_message(MessageHandle::new(AutoReceiveMessage::PostStart), d)
      .await;

    for i in 0..10 {
      tracing::debug!("Sending fail message: {}", i);
      root_context.send(child.clone(), fail.clone()).await;
      observer.expect_message(fail.clone(), d).await.unwrap();
      observer
        .expect_message(MessageHandle::new(AutoReceiveMessage::PreRestart), d)
        .await
        .unwrap();
      observer
        .expect_message(MessageHandle::new(AutoReceiveMessage::PostRestart), d)
        .await
        .unwrap();
    }
    root_context.send(child, fail.clone()).await;
    observer.expect_message(fail.clone(), d).await.unwrap();
    observer
      .expect_message(MessageHandle::new(AutoReceiveMessage::PreStop), d)
      .await
      .unwrap();
  }

  #[derive(Debug, Clone)]
  struct ActorWithSupervisor {
    notify: Arc<Notify>,
  }

  #[derive(Debug, Clone)]
  struct FailingChildActor;

  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  struct StringMessage(String);

  #[async_trait]
  impl Actor for ActorWithSupervisor {
    async fn post_start(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("ActorWithSupervisor::post_start");
      let props = Props::from_async_actor_producer(|_| async { FailingChildActor }).await;
      let child = ctx.spawn(props).await;
      ctx
        .send(child, MessageHandle::new(StringMessage("fail".to_string())))
        .await;
      Ok(())
    }

    async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("ActorWithSupervisor::receive");
      Ok(())
    }

    async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
      Some(SupervisorStrategyHandle::new(self.clone()))
    }
  }

  #[async_trait]
  impl SupervisorStrategy for ActorWithSupervisor {
    async fn handle_child_failure(
      &self,
      _: ActorSystem,
      _: SupervisorHandle,
      child: CorePid,
      rs: RestartStatistics,
      _: ErrorReason,
      message_handle: MessageHandle,
    ) {
      tracing::debug!(
        "ActorWithSupervisor::handle_failure: child = {}, rs = {}, message = {:?}",
        child,
        rs,
        message_handle
      );
      self.notify.notify_one();
    }

    fn as_any(&self) -> &dyn Any {
      self
    }
  }

  #[async_trait]
  impl Actor for FailingChildActor {
    async fn post_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("FailingChildActor::post_start");
      Ok(())
    }

    async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
      let message_handle = ctx.get_message_handle_opt().await.expect("message not found");
      tracing::debug!("FailingChildActor::receive: msg = {:?}", message_handle);
      if let Some(StringMessage(msg)) = message_handle.to_typed::<StringMessage>() {
        tracing::debug!("FailingChildActor::receive: msg = {:?}", msg);
        Err(ActorError::ReceiveError(ErrorReason::new("error", 0)))
      } else {
        Ok(())
      }
    }
  }
  #[derive(Debug, Error)]
  enum TestError {
    #[error("Timeout")]
    TimeoutError,
    #[error("NoMatch")]
    NoMatch,
  }

  #[derive(Debug, Clone)]
  struct Observer {
    received: Arc<Mutex<VecDeque<MessageHandle>>>,
  }

  impl Observer {
    fn new() -> Self {
      Observer {
        received: Arc::new(Mutex::new(VecDeque::new())),
      }
    }

    async fn receive(&self, _: ReceiverContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
      tracing::debug!(">>> Observer::receive: message_handle = {:?}", message_handle);
      self.received.lock().await.push_back(message_handle);
      Ok(())
    }

    async fn expect_message(&self, expected: MessageHandle, timeout: Duration) -> Result<(), TestError> {
      let start = Instant::now();
      let mut counter = 0;
      while start.elapsed() <= timeout {
        counter += 1;
        if counter == 1 {
          tracing::debug!("expect_message: expected = {:?}, counter = {}", expected, counter);
        }
        if let Some(received) = self.received.lock().await.pop_front() {
          tracing::debug!("expected = {:?}, received = {:?}", expected, received);
          if expected.eq_message(&received) {
            return Ok(());
          } else {
            return Err(TestError::NoMatch);
          }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
      Err(TestError::TimeoutError)
    }
  }
}
