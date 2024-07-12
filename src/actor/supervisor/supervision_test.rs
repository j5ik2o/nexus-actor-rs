#[cfg(test)]
mod test {
  use async_trait::async_trait;
  use std::any::Any;
  use std::collections::VecDeque;
  use std::env;
  use std::sync::Arc;
  use std::time::Duration;
  use thiserror::Error;
  use tokio::sync::{Mutex, Notify};
  use tokio::time::Instant;
  use tracing_subscriber::EnvFilter;

  use crate::actor::actor::actor::Actor;
  use crate::actor::actor::actor_error::ActorError;
  use crate::actor::actor::actor_inner_error::ActorInnerError;
  use crate::actor::actor::pid::ExtendedPid;
  use crate::actor::actor::props::Props;
  use crate::actor::actor::receiver_middleware::ReceiverMiddleware;
  use crate::actor::actor::receiver_middleware_chain::ReceiverMiddlewareChain;
  use crate::actor::actor::restart_statistics::RestartStatistics;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::context_handle::ContextHandle;
  use crate::actor::context::receiver_context_handle::ReceiverContextHandle;
  use crate::actor::context::{SenderPart, SpawnerPart};
  use crate::actor::message::auto_receive_message::AutoReceiveMessage;
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::system_message::SystemMessage;
  use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;
  use crate::actor::supervisor::supervisor_strategy::{SupervisorHandle, SupervisorStrategy};
  use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

  #[tokio::test]
  async fn test_actor_with_own_supervisor_can_handle_failure() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await;
    let mut root = system.get_root_context().await;
    let notify = Arc::new(Notify::new());
    let cloned_notify = notify.clone();
    let props = Props::from_actor_producer(move |_| {
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
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();
    let observer = Observer::new();
    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let cloned_observer = observer.clone();
    let middles = ReceiverMiddleware::new(move |next| {
      let cloned_observer = cloned_observer.clone();
      ReceiverMiddlewareChain::new(move |ctx, moe| {
        let next = next.clone();
        let cloned_observer = cloned_observer.clone();
        async move {
          tracing::debug!("ReceiverMiddleware: moe = {:?}", moe);
          let msg = moe.get_message_handle();
          tracing::debug!(">>>> msg = {:?}", msg);
          let result = cloned_observer.receive(ctx.clone(), msg.clone()).await;
          if result.is_err() {
            return result;
          }
          next.run(ctx, moe).await
        }
      })
    });

    let props = Props::from_actor_producer_with_opts(
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
      .expect_message(MessageHandle::new(SystemMessage::Started), d)
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
        .expect_message(MessageHandle::new(SystemMessage::Started), d)
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

  #[derive(Debug, Clone)]
  struct StringMessage(String);

  impl PartialEq for StringMessage {
    fn eq(&self, other: &Self) -> bool {
      self.0 == other.0
    }
  }

  impl Message for StringMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      let other = other.as_any().downcast_ref::<StringMessage>();
      match other {
        Some(other) => self.0 == other.0,
        None => false,
      }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[async_trait]
  impl Actor for ActorWithSupervisor {
    async fn post_start(&self, mut ctx: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("ActorWithSupervisor::post_start");
      let props = Props::from_actor_producer(|_| async { FailingChildActor }).await;
      let child = ctx.spawn(props).await;
      ctx
        .send(child, MessageHandle::new(StringMessage("fail".to_string())))
        .await;
      Ok(())
    }

    async fn receive(&mut self, _: ContextHandle, _: MessageHandle) -> Result<(), ActorError> {
      tracing::debug!("ActorWithSupervisor::receive");
      Ok(())
    }

    async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
      Some(SupervisorStrategyHandle::new(self.clone()))
    }
  }

  #[async_trait]
  impl SupervisorStrategy for ActorWithSupervisor {
    async fn handle_child_failure(
      &self,
      _: &ActorSystem,
      _: SupervisorHandle,
      child: ExtendedPid,
      rs: RestartStatistics,
      _: ActorInnerError,
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
    async fn post_start(&self, _: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("FailingChildActor::started");
      Ok(())
    }

    async fn receive(&mut self, _: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError> {
      tracing::debug!("FailingChildActor::receive: msg = {:?}", message_handle);
      if let Some(StringMessage(msg)) = message_handle.to_typed::<StringMessage>() {
        tracing::debug!("FailingChildActor::receive: msg = {:?}", msg);
        Err(ActorError::ReceiveError(ActorInnerError::new("error")))
      } else {
        Ok(())
      }
    }
  }
  #[derive(Debug, Error)]
  enum TestError {
    #[error("Timeout")]
    TimeoutError,
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
      self.received.lock().await.push_back(message_handle);
      Ok(())
    }

    async fn expect_message(&self, expected: MessageHandle, timeout: Duration) -> Result<(), TestError> {
      let start = Instant::now();
      while start.elapsed() <= timeout {
        if let Some(received) = self.received.lock().await.pop_front() {
          tracing::debug!("expected = {:?}, received = {:?}", expected, received);
          if expected == received {
            return Ok(());
          }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
      Err(TestError::TimeoutError)
    }
  }
}
