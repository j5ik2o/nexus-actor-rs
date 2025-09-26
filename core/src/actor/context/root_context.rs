use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::context::sender_context_handle::SenderContextHandle;
use crate::actor::context::spawner_context_handle::SpawnerContextHandle;
use crate::actor::context::{
  InfoPart, MessagePart, SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart, TypedRootContext,
};
use crate::actor::core::make_sender_middleware_chain;
use crate::actor::core::ActorHandle;
use crate::actor::core::ExtendedPid;
use crate::actor::core::Props;
use crate::actor::core::SenderMiddleware;
use crate::actor::core::SenderMiddlewareChain;
use crate::actor::core::SpawnError;
use crate::actor::core::Spawner;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::MessageHeaders;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::SystemMessage;
use crate::actor::process::actor_future::ActorFuture;
use crate::actor::process::future::ActorFutureProcess;
use crate::actor::process::Process;
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::generated::actor::{PoisonPill, Watch};

fn ensure_envelope(message_handle: MessageHandle) -> MessageEnvelope {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.clone()
  } else {
    MessageEnvelope::new(message_handle)
  }
}

#[derive(Debug, Clone)]
pub struct RootContext {
  actor_system: WeakActorSystem,
  sender_middleware_chain: Option<SenderMiddlewareChain>,
  spawn_middleware: Option<Spawner>,
  message_headers: Arc<MessageHeaders>,
  guardian_strategy: Option<SupervisorStrategyHandle>,
}

#[derive(Debug, Clone)]
pub struct RootSendPipeline {
  actor_system: ActorSystem,
  target: ExtendedPid,
  message_handle: MessageHandle,
  middleware_chain: Option<SenderMiddlewareChain>,
  root_context: RootContext,
}

impl RootSendPipeline {
  pub fn actor_system(&self) -> &ActorSystem {
    &self.actor_system
  }

  pub fn target(&self) -> &ExtendedPid {
    &self.target
  }

  pub fn message_handle(&self) -> &MessageHandle {
    &self.message_handle
  }

  pub async fn dispatch(self) {
    let RootSendPipeline {
      actor_system,
      target,
      message_handle,
      middleware_chain,
      root_context,
    } = self;

    if let Some(chain) = middleware_chain {
      let sender_context = SenderContextHandle::from_root(root_context);
      let envelope = ensure_envelope(message_handle);
      chain.run(sender_context, target, envelope).await;
    } else {
      target.send_user_message(actor_system, message_handle).await;
    }
  }
}

#[derive(Debug, Clone)]
pub struct RootRequestFuturePipeline {
  actor_system: ActorSystem,
  target: ExtendedPid,
  message_handle: MessageHandle,
  timeout: Duration,
  middleware_chain: Option<SenderMiddlewareChain>,
  root_context: RootContext,
}

impl RootRequestFuturePipeline {
  pub fn timeout(&self) -> Duration {
    self.timeout
  }

  pub async fn dispatch(self) -> ActorFuture {
    let RootRequestFuturePipeline {
      actor_system,
      target,
      message_handle,
      timeout,
      middleware_chain,
      root_context,
    } = self;

    let future_process = ActorFutureProcess::new(actor_system.clone(), timeout).await;
    let future_pid = future_process.get_pid().await;
    let envelope = ensure_envelope(message_handle).with_sender(future_pid.clone());

    RootSendPipeline {
      actor_system,
      target,
      message_handle: MessageHandle::new(envelope),
      middleware_chain,
      root_context,
    }
    .dispatch()
    .await;

    future_process.get_future().await
  }
}

impl RootContext {
  pub fn new(actor_system: ActorSystem, headers: Arc<MessageHeaders>, sender_middleware: &[SenderMiddleware]) -> Self {
    let weak_system = actor_system.downgrade();
    let sender_middleware_chain = make_sender_middleware_chain(
      sender_middleware,
      SenderMiddlewareChain::new({
        let weak_system = weak_system.clone();
        move |_, target, envelope| {
          let weak_system = weak_system.clone();
          async move {
            let actor_system = weak_system
              .upgrade()
              .expect("ActorSystem dropped before RootContext sender middleware");
            target
              .send_user_message(actor_system, envelope.get_message_handle())
              .await
          }
        }
      }),
    );
    Self {
      actor_system: weak_system,
      sender_middleware_chain,
      spawn_middleware: None,
      message_headers: headers,
      guardian_strategy: None,
    }
  }

  pub fn with_actor_system(mut self, actor_system: ActorSystem) -> Self {
    self.actor_system = actor_system.downgrade();
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

  pub fn actor_system_snapshot(&self) -> ActorSystem {
    self.actor_system()
  }

  pub fn message_headers_snapshot(&self) -> Arc<MessageHeaders> {
    self.message_headers.clone()
  }

  async fn send_user_message(&self, pid: ExtendedPid, message_handle: MessageHandle) {
    if self.sender_middleware_chain.is_some() {
      let sch = SenderContextHandle::from_root(self.clone());
      let me = MessageEnvelope::new(message_handle);
      self.sender_middleware_chain.clone().unwrap().run(sch, pid, me).await;
    } else {
      tracing::debug!("Sending user message to pid: {}", pid);
      let actor_system = self.actor_system();
      pid.send_user_message(actor_system, message_handle).await;
    }
  }

  pub fn prepare_send(&self, pid: ExtendedPid, message_handle: MessageHandle) -> RootSendPipeline {
    RootSendPipeline {
      actor_system: self.actor_system(),
      target: pid,
      message_handle,
      middleware_chain: self.sender_middleware_chain.clone(),
      root_context: self.clone(),
    }
  }

  pub fn prepare_request_with_sender(
    &self,
    pid: ExtendedPid,
    message_handle: MessageHandle,
    sender: ExtendedPid,
  ) -> RootSendPipeline {
    let envelope = ensure_envelope(message_handle).with_sender(sender);
    self.prepare_send(pid, MessageHandle::new(envelope))
  }

  pub fn prepare_request_future(
    &self,
    pid: ExtendedPid,
    message_handle: MessageHandle,
    timeout: Duration,
  ) -> RootRequestFuturePipeline {
    RootRequestFuturePipeline {
      actor_system: self.actor_system(),
      target: pid,
      message_handle,
      timeout,
      middleware_chain: self.sender_middleware_chain.clone(),
      root_context: self.clone(),
    }
  }

  pub fn to_typed(self) -> TypedRootContext {
    TypedRootContext::new(self)
  }

  fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before RootContext")
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
    self.actor_system()
  }
}

#[async_trait]
impl SenderPart for RootContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    None
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    self.prepare_send(pid, message_handle).dispatch().await
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    self.prepare_send(pid, message_handle).dispatch().await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    self
      .prepare_request_with_sender(pid, message_handle, sender)
      .dispatch()
      .await
  }

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    self
      .prepare_request_future(pid, message_handle, timeout)
      .dispatch()
      .await
  }
}

#[async_trait]
impl MessagePart for RootContext {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    None
  }

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

    if let Some(sm) = &root_context.spawn_middleware {
      let sh = SpawnerContextHandle::new(root_context.clone());
      let actor_system = self.actor_system();
      return sm.run(actor_system, id, props.clone(), sh).await;
    }

    let actor_system = self.actor_system();
    props
      .clone()
      .spawn(actor_system, id, SpawnerContextHandle::new(root_context.clone()))
      .await
  }
}

impl SpawnerContext for RootContext {}

#[async_trait]
impl StopperPart for RootContext {
  async fn stop(&mut self, pid: &ExtendedPid) {
    let actor_system = self.actor_system();
    pid.ref_process(actor_system.clone()).await.stop(pid).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let actor_system = self.actor_system();
    let future_process = ActorFutureProcess::new(actor_system.clone(), timeout).await;

    let future_pid = future_process.get_pid().await.clone();
    pid
      .send_system_message(
        actor_system,
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_pid.inner_pid),
        })),
      )
      .await;
    self.stop(pid).await;

    future_process.get_future().await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let actor_system = self.actor_system();
    pid
      .send_user_message(actor_system, MessageHandle::new(PoisonPill {}))
      .await
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let actor_system = self.actor_system();
    let future_process = ActorFutureProcess::new(actor_system.clone(), timeout).await;

    let future_pid = future_process.get_pid().await.clone();
    pid
      .send_system_message(
        actor_system,
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_pid.inner_pid),
        })),
      )
      .await;
    self.poison(pid).await;

    future_process.get_future().await
  }
}
