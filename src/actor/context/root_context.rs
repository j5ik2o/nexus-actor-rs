use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::actor::actor::actor_handle::ActorHandle;
use crate::actor::actor::middleware_chain::make_sender_middleware_chain;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::props::Props;
use crate::actor::actor::sender_middleware::SenderMiddleware;
use crate::actor::actor::sender_middleware_chain::SenderMiddlewareChain;
use crate::actor::actor::spawner::SpawnError;
use crate::actor::actor::spawner::Spawner;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::sender_context_handle::SenderContextHandle;
use crate::actor::context::spawner_context_handle::SpawnerContextHandle;
use crate::actor::context::{
  InfoPart, MessagePart, SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::future::{Future, FutureProcess};
use crate::actor::message::auto_receive_message::AutoReceiveMessage;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::message_headers::MessageHeaders;
use crate::actor::message::message_or_envelope::MessageEnvelope;
use crate::actor::message::readonly_message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::message::system_message::SystemMessage;
use crate::actor::message::watch::Watch;
use crate::actor::process::Process;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

#[derive(Debug, Clone)]
pub struct RootContext {
  actor_system: ActorSystem,
  sender_middleware_chain: Option<SenderMiddlewareChain>,
  spawn_middleware: Option<Spawner>,
  message_headers: Arc<MessageHeaders>,
  guardian_strategy: Option<SupervisorStrategyHandle>,
}

impl RootContext {
  pub fn new(actor_system: ActorSystem, headers: Arc<MessageHeaders>, sender_middleware: &[SenderMiddleware]) -> Self {
    Self {
      actor_system: actor_system.clone(),
      sender_middleware_chain: make_sender_middleware_chain(
        &sender_middleware,
        SenderMiddlewareChain::new(move |_, target, envelope| {
          let actor_system = actor_system.clone();
          async move {
            target
              .send_user_message(actor_system, envelope.get_message_handle().clone())
              .await
          }
        }),
      ),
      spawn_middleware: None,
      message_headers: headers,
      guardian_strategy: None,
    }
  }

  pub fn with_actor_system(mut self, actor_system: ActorSystem) -> Self {
    self.actor_system = actor_system;
    self
  }

  pub fn with_guardian(mut self, strategy: SupervisorStrategyHandle) -> Self {
    self.guardian_strategy = Some(strategy);
    self
  }

  pub fn with_headers(mut self, headers: Arc<MessageHeaders>) -> Self {
    self.message_headers = headers;
    self
  }

  async fn send_user_message(&self, pid: ExtendedPid, message_handle: MessageHandle) {
    if self.sender_middleware_chain.is_some() {
      let sch = SenderContextHandle::new(self.clone());
      let me = MessageEnvelope::new(message_handle);
      self.sender_middleware_chain.clone().unwrap().run(sch, pid, me).await;
    } else {
      pid.send_user_message(self.actor_system.clone(), message_handle).await;
    }
  }
}

#[async_trait]
impl InfoPart for RootContext {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    None
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    if self.guardian_strategy.is_some() {
      let ssh = self.guardian_strategy.clone().unwrap();
      Some(
        self
          .get_actor_system()
          .await
          .get_guardians()
          .await
          .get_guardian_pid(ssh)
          .await,
      )
    } else {
      None
    }
  }

  async fn set_self(&mut self, _pid: ExtendedPid) {}

  async fn get_actor(&self) -> Option<ActorHandle> {
    None
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.actor_system.clone()
  }
}

#[async_trait]
impl SenderPart for RootContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    None
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    self.send_user_message(pid, message_handle).await
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    self.send_user_message(pid, message_handle).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    self
      .send_user_message(
        pid,
        MessageHandle::new(MessageEnvelope::new(message_handle).with_sender(sender)),
      )
      .await
  }

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> Future {
    let future_process = FutureProcess::new(self.get_actor_system().await, timeout).await;
    let future_pid = future_process.get_pid().await;
    let moe = MessageEnvelope::new(message_handle).with_sender(future_pid.clone());
    self.send_user_message(pid, MessageHandle::new(moe)).await;
    future_process.get_future().await
  }
}

#[async_trait]
impl MessagePart for RootContext {
  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    None
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    Some(ReadonlyMessageHeadersHandle::new_arc(self.message_headers.clone()))
  }
}

impl SenderContext for RootContext {}

#[async_trait]
impl SpawnerPart for RootContext {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    match self
      .spawn_named(
        props,
        &self.get_actor_system().await.get_process_registry().await.next_id(),
      )
      .await
    {
      Ok(pid) => pid,
      Err(e) => panic!("Failed to spawn actor: {:?}", e),
    }
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    match self
      .spawn_named(
        props,
        &format!(
          "{}-{}",
          prefix,
          self.get_actor_system().await.get_process_registry().await.next_id()
        ),
      )
      .await
    {
      Ok(pid) => pid,
      Err(e) => panic!("Failed to spawn actor: {:?}", e),
    }
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let mut root_context = self.clone();
    if self.guardian_strategy.is_some() {
      root_context = root_context.with_guardian(self.guardian_strategy.clone().unwrap());
    }

    match &root_context.spawn_middleware {
      Some(sm) => {
        let sh = SpawnerContextHandle::new(root_context.clone());
        return sm
          .run(self.get_actor_system().await.clone(), id, props.clone(), sh)
          .await;
      }
      _ => {}
    }

    props
      .clone()
      .spawn(
        self.get_actor_system().await.clone(),
        id,
        SpawnerContextHandle::new(root_context.clone()),
      )
      .await
  }
}

impl SpawnerContext for RootContext {}

#[async_trait]
impl StopperPart for RootContext {
  async fn stop(&mut self, pid: &ExtendedPid) {
    pid.ref_process(self.get_actor_system().await).await.stop(pid).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> Future {
    let future_process = FutureProcess::new(self.get_actor_system().await, timeout).await;

    let future_pid = future_process.get_pid().await.clone();
    pid
      .send_system_message(
        self.get_actor_system().await.clone(),
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_pid.inner_pid),
        })),
      )
      .await;
    self.stop(pid).await;

    future_process.get_future().await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    pid
      .send_user_message(
        self.get_actor_system().await.clone(),
        MessageHandle::new(AutoReceiveMessage::PoisonPill),
      )
      .await
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> Future {
    let future_process = FutureProcess::new(self.get_actor_system().await, timeout).await;

    let future_pid = future_process.get_pid().await.clone();
    pid
      .send_system_message(
        self.get_actor_system().await.clone(),
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_pid.inner_pid),
        })),
      )
      .await;
    self.poison(pid).await;

    future_process.get_future().await
  }
}
