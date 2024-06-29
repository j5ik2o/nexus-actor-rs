use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::actor::actor::PoisonPill;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{
  InfoPart, MessagePart, SenderContext, SenderContextHandle, SenderPart, SpawnerContext, SpawnerContextHandle,
  SpawnerPart, StopperPart,
};
use crate::actor::future::Future;
use crate::actor::message::{ActorHandle, MessageHandle, SenderFunc};
use crate::actor::message_envelope::{MessageEnvelope, MessageHeaders, ReadonlyMessageHeadersHandle};
use crate::actor::messages::AutoReceiveMessage;
use crate::actor::middleware_chain::make_sender_middleware_chain;
use crate::actor::pid::ExtendedPid;
use crate::actor::process::Process;
use crate::actor::props::{Props, SenderMiddleware, SpawnError, SpawnFunc};
use crate::actor::supervisor_strategy::SupervisorStrategyHandle;

#[derive(Debug, Clone)]
pub struct RootContext {
  actor_system: ActorSystem,
  sender_middleware: Option<SenderFunc>,
  spawn_middleware: Option<SpawnFunc>,
  headers: Arc<MessageHeaders>,
  guardian_strategy: Option<SupervisorStrategyHandle>,
}

impl RootContext {
  pub fn new(actor_system: ActorSystem, headers: Arc<MessageHeaders>, sender_middleware: &[SenderMiddleware]) -> Self {
    Self {
      actor_system: actor_system.clone(),
      sender_middleware: make_sender_middleware_chain(
        &sender_middleware,
        SenderFunc::new(move |_, target, envelope| {
          let actor_system = actor_system.clone();
          Box::pin(async move { target.send_user_message(actor_system, envelope.message.clone()).await })
        }),
      ),
      spawn_middleware: None,
      headers,
      guardian_strategy: None,
    }
  }

  pub fn with_guardian(mut self, strategy: SupervisorStrategyHandle) -> Self {
    self.guardian_strategy = Some(strategy);
    self
  }

  async fn send_user_message(&self, pid: ExtendedPid, message: MessageHandle) {
    if self.sender_middleware.is_some() {
      let sch = SenderContextHandle::new(self.clone());
      let me = MessageEnvelope::new(message);
      self.sender_middleware.clone().unwrap().run(sch, pid, me).await;
    } else {
      pid.send_user_message(self.actor_system.clone(), message).await;
    }
  }
}

#[async_trait]
impl InfoPart for RootContext {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    None
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    if self.guardian_strategy.is_some() {
      let ssh = SupervisorStrategyHandle::new(self.guardian_strategy.clone().unwrap());
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

  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle) {
    self.send_user_message(pid, message).await
  }

  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle) {
    self.send_user_message(pid, message).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid) {
    self
      .send_user_message(
        pid,
        MessageHandle::new(MessageEnvelope::new(message).with_sender(sender)),
      )
      .await
  }

  async fn request_future(&self, _pid: ExtendedPid, _message: MessageHandle, _timeout: &Duration) -> Future {
    todo!()
  }
}

#[async_trait]
impl MessagePart for RootContext {
  async fn get_message(&self) -> Option<MessageHandle> {
    None
  }

  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    ReadonlyMessageHeadersHandle::new_arc(self.headers.clone())
  }
}

impl SenderContext for RootContext {}

#[async_trait]
impl SpawnerPart for RootContext {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let result = self
      .spawn_named(
        props,
        &self.get_actor_system().await.get_process_registry().await.next_id(),
      )
      .await;
    match result {
      Ok(pid) => pid,
      Err(e) => panic!("Failed to spawn actor: {:?}", e),
    }
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let result = self
      .spawn_named(
        props,
        &format!(
          "{}-{}",
          prefix,
          self.get_actor_system().await.get_process_registry().await.next_id()
        ),
      )
      .await;
    match result {
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

    let sh = SpawnerContextHandle::new(root_context.clone());
    props
      .clone()
      .spawn(self.get_actor_system().await.clone(), id, sh)
      .await
  }
}

impl SpawnerContext for RootContext {}

#[async_trait]
impl StopperPart for RootContext {
  async fn stop(&mut self, pid: &ExtendedPid) {
    pid.ref_process(self.get_actor_system().await).await.stop(pid).await
  }

  async fn stop_future(&mut self, _: &ExtendedPid) -> Future {
    todo!()
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    pid
      .send_user_message(
        self.get_actor_system().await.clone(),
        MessageHandle::new(AutoReceiveMessage::PoisonPill(PoisonPill {})),
      )
      .await
  }

  async fn poison_future(&mut self, pid: &ExtendedPid) -> Future {
    todo!()
  }
}
