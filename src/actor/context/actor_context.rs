use std::any::Any;
use std::error::Error;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use tokio::sync::Mutex;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::pid_set::PidSet;
use crate::actor::actor::props::{Props, SpawnError};
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor::{
  Actor, ActorError, ActorHandle, ActorInnerError, PoisonPill, Stop, Terminated, Unwatch, Watch,
};
use crate::actor::actor_system::ActorSystem;
use crate::actor::auto_respond::{AutoRespond, AutoRespondHandle};
use crate::actor::context::{
  BasePart, Context, ContextHandle, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext,
  ReceiverContextHandle, ReceiverPart, SenderContext, SenderContextHandle, SenderPart, SpawnerContext,
  SpawnerContextHandle, SpawnerPart, StopperPart,
};
use crate::actor::dispatch::message_invoker::MessageInvoker;
use crate::actor::future::Future;
use crate::actor::log::P_LOG;
use crate::actor::message::{
  Message, MessageHandle, MessageHandles, ProducerFunc, ReceiverFunc, ResponseHandle, SenderFunc,
};
use crate::actor::message_envelope::{MessageEnvelope, MessageOrEnvelope, ReadonlyMessageHeadersHandle};
use crate::actor::messages::{
  AutoReceiveMessage, Continuation, Failure, MailboxMessage, NotInfluenceReceiveTimeoutHandle, ReceiveTimeout, Restart,
  Restarting, Started, Stopped, Stopping, SystemMessage,
};
use crate::actor::process::Process;
use crate::actor::supervisor::supervisor_strategy::{
  Supervisor, SupervisorHandle, SupervisorStrategy, DEFAULT_SUPERVISION_STRATEGY,
};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId, ContextExtensions};

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum State {
  Alive = 0,
  Restarting = 1,
  Stopping = 2,
  Stopped = 3,
}

#[derive(Debug, Clone)]
pub struct ActorContextExtrasInner {
  children: PidSet,
  pub(crate) receive_timeout_timer: Option<ReceiveTimeoutTimer>,
  rs: Arc<Mutex<Option<RestartStatistics>>>,
  stash: MessageHandles,
  watchers: PidSet,
  context: ContextHandle,
  extensions: ContextExtensions,
}

impl ActorContextExtrasInner {
  pub async fn new(context: ContextHandle) -> Self {
    ActorContextExtrasInner {
      children: PidSet::new(&[]).await,
      receive_timeout_timer: None,
      rs: Arc::new(Mutex::new(None)),
      stash: MessageHandles::new(vec![]),
      watchers: PidSet::new(&[]).await,
      context,
      extensions: ContextExtensions::new(),
    }
  }
}
#[derive(Debug, Clone)]
pub struct ActorContextExtras {
  inner: Arc<Mutex<ActorContextExtrasInner>>,
}

unsafe impl Send for ActorContextExtras {}
unsafe impl Sync for ActorContextExtras {}

#[derive(Debug, Clone)]
pub struct SleepContainer(Arc<Mutex<Pin<Box<tokio::time::Sleep>>>>);
impl SleepContainer {
  pub fn from_sleep(sleep: tokio::time::Sleep) -> Self {
    SleepContainer(Arc::new(Mutex::new(Box::pin(sleep))))
  }

  pub fn new(duration: tokio::time::Duration) -> Self {
    Self::from_sleep(tokio::time::sleep(duration))
  }

  pub fn from_underlying(underlying: Arc<Mutex<Pin<Box<tokio::time::Sleep>>>>) -> Self {
    Self(underlying)
  }

  pub async fn init(&mut self, instant: tokio::time::Instant) {
    let mut timer = self.0.lock().await;
    *timer = Box::pin(tokio::time::sleep_until(instant));
  }

  pub async fn reset(&mut self, instant: tokio::time::Instant) {
    let mut sleep = self.0.lock().await;
    sleep.as_mut().reset(instant);
  }

  pub async fn stop(&mut self) {
    let mut sleep = self.0.lock().await;
    sleep.as_mut().reset(tokio::time::Instant::now());
  }

  pub async fn wait(&self) {
    let mut sleep = self.0.lock().await;
    sleep.as_mut().await;
  }
}

#[derive(Debug, Clone)]
pub struct ReceiveTimeoutTimer(SleepContainer);

impl ReceiveTimeoutTimer {
  pub fn new(duration: tokio::time::Duration) -> Self {
    ReceiveTimeoutTimer(SleepContainer::new(duration))
  }

  pub fn from_sleep(sleep: tokio::time::Sleep) -> Self {
    ReceiveTimeoutTimer(SleepContainer::from_sleep(sleep))
  }

  pub fn from_underlying(underlying: Arc<Mutex<Pin<Box<tokio::time::Sleep>>>>) -> Self {
    ReceiveTimeoutTimer(SleepContainer::from_underlying(underlying))
  }

  pub async fn reset(&mut self, instant: tokio::time::Instant) {
    self.0.reset(instant).await;
  }

  pub async fn init(&mut self, instant: tokio::time::Instant) {
    self.0.init(instant).await;
  }

  pub async fn stop(&mut self) {
    self.0.stop().await;
  }

  pub async fn wait(&self) {
    self.0.wait().await;
  }
}

impl ActorContextExtras {
  pub async fn new(context: ContextHandle) -> Self {
    ActorContextExtras {
      inner: Arc::new(Mutex::new(ActorContextExtrasInner::new(context).await)),
    }
  }

  pub async fn get_receive_timeout_timer(&self) -> Option<ReceiveTimeoutTimer> {
    let mg = self.inner.lock().await;
    mg.receive_timeout_timer.clone()
  }

  pub async fn get_context(&self) -> ContextHandle {
    let mg = self.inner.lock().await;
    mg.context.clone()
  }

  pub async fn get_sender_context(&self) -> SenderContextHandle {
    let inner_mg = self.inner.lock().await;
    SenderContextHandle::new(inner_mg.context.clone())
  }

  pub async fn get_receiver_context(&self) -> ReceiverContextHandle {
    let inner_mg = self.inner.lock().await;
    ReceiverContextHandle::new(inner_mg.context.clone())
  }

  pub async fn get_extensions(&self) -> ContextExtensions {
    let inner_mg = self.inner.lock().await;
    inner_mg.extensions.clone()
  }

  pub async fn get_children(&self) -> PidSet {
    let inner_mg = self.inner.lock().await;
    inner_mg.children.clone()
  }

  pub async fn get_watchers(&self) -> PidSet {
    let inner_mg = self.inner.lock().await;
    inner_mg.watchers.clone()
  }

  pub async fn get_stash(&self) -> MessageHandles {
    let inner_mg = self.inner.lock().await;
    inner_mg.stash.clone()
  }

  pub async fn restart_stats(&mut self) -> RestartStatistics {
    let inner_mg = self.inner.lock().await;
    let mut rs_mg = inner_mg.rs.lock().await;
    if rs_mg.is_none() {
      *rs_mg = Some(RestartStatistics::new())
    }
    rs_mg.as_ref().unwrap().clone()
  }

  pub async fn init_receive_timeout_timer(&self, duration: tokio::time::Duration) {
    let mut inner_mg = self.inner.lock().await;
    match inner_mg.receive_timeout_timer {
      Some(_) => return,
      None => {
        inner_mg.receive_timeout_timer = Some(ReceiveTimeoutTimer::new(duration));
      }
    }
  }

  async fn init_or_reset_receive_timeout_timer(&mut self, d: tokio::time::Duration, context: Arc<Mutex<ActorContext>>) {
    self.stop_receive_timeout_timer().await;

    let timer = Arc::new(Mutex::new(Box::pin(tokio::time::sleep(d))));
    {
      let mut mg = self.inner.lock().await;
      mg.receive_timeout_timer = Some(ReceiveTimeoutTimer::from_underlying(timer.clone()));
    }

    let context = context.clone();
    tokio::spawn(async move {
      let mut mg = timer.lock().await;
      mg.as_mut().await;
      let mut locked_context = context.lock().await;
      locked_context.receive_timeout_handler().await;
    });
  }

  pub async fn reset_receive_timeout_timer(&self, duration: tokio::time::Duration) {
    let mut mg = self.inner.lock().await;
    if let Some(t) = &mut mg.receive_timeout_timer {
      t.reset(tokio::time::Instant::now() + duration).await;
    }
  }

  pub async fn stop_receive_timeout_timer(&self) {
    let mut mg = self.inner.lock().await;
    if let Some(t) = &mut mg.receive_timeout_timer {
      t.stop().await;
    }
  }

  pub async fn kill_receive_timeout_timer(&self) {
    let mut mg = self.inner.lock().await;
    let timer = mg.receive_timeout_timer.clone();
    if timer.is_some() {
      mg.receive_timeout_timer = None
    }
  }

  pub async fn wait_for_timeout(&self) {
    let mg = self.inner.lock().await;
    if let Some(timer) = mg.receive_timeout_timer.clone() {
      timer.wait().await;
    }

    if let Some(t) = &mg.receive_timeout_timer {
      t.wait().await
    }
  }

  pub async fn add_child(&mut self, pid: ExtendedPid) {
    let mut mg = self.inner.lock().await;
    mg.children.add(pid).await;
  }

  pub async fn remove_child(&mut self, pid: &ExtendedPid) {
    let mut mg = self.inner.lock().await;
    mg.children.remove(pid).await;
  }
}

#[derive(Debug, Clone)]
pub struct ActorContextInner {
  actor: Option<ActorHandle>,
  actor_system: ActorSystem,
  extras: Arc<Mutex<Option<ActorContextExtras>>>,
  props: Props,
  parent: Option<ExtendedPid>,
  self_pid: Option<ExtendedPid>,
  receive_timeout: Option<tokio::time::Duration>,
  producer: Option<ProducerFunc>,
  message_or_envelope: Option<MessageOrEnvelope>,
  state: Option<Arc<AtomicU8>>,
}

#[derive(Debug, Clone)]
pub struct ActorContext {
  inner: Arc<Mutex<ActorContextInner>>,
}

impl PartialEq for ActorContext {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl Eq for ActorContext {}

impl std::hash::Hash for ActorContext {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.inner.as_ref() as *const Mutex<ActorContextInner>).hash(state);
  }
}

unsafe impl Send for ActorContext {}
unsafe impl Sync for ActorContext {}

impl ActorContext {
  pub async fn new(actor_system: ActorSystem, props: Props, parent: Option<ExtendedPid>) -> Self {
    let mut ctx = ActorContext {
      inner: Arc::new(Mutex::new(ActorContextInner {
        actor: None,
        actor_system,
        extras: Arc::new(Mutex::new(None)),
        props,
        parent,
        self_pid: None,
        receive_timeout: None,
        producer: None,
        message_or_envelope: None,
        state: None,
      })),
    };
    ctx.incarnate_actor().await;
    ctx
  }

  async fn get_extras(&self) -> Option<ActorContextExtras> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.extras.lock().await;
    mg.clone()
  }

  async fn set_extras(&mut self, extras: Option<ActorContextExtras>) {
    let inner_mg = self.inner.lock().await;
    *inner_mg.extras.lock().await = extras;
  }

  async fn get_props(&self) -> Props {
    let inner_mg = self.inner.lock().await;
    inner_mg.props.clone()
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor.clone()
  }

  async fn set_actor(&mut self, actor: Option<ActorHandle>) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.actor = actor;
  }

  async fn receive_timeout_handler(&mut self) {
    if let Some(extras) = self.get_extras().await {
      if extras.get_receive_timeout_timer().await.is_some() {
        self.cancel_receive_timeout().await;
        self
          .send(self.get_self().await.unwrap(), MessageHandle::new(ReceiveTimeout {}))
          .await;
      }
    }
  }

  async fn ensure_extras(&mut self) -> ActorContextExtras {
    if self.get_extras().await.is_none() {
      let ctxd = self.clone();
      let ctxd = if let Some(decorator) = &self.get_props().await.get_context_decorator_chain() {
        decorator.run(ContextHandle::new(ctxd)).await
      } else {
        ContextHandle::new(ctxd)
      };
      self.set_extras(Some(ActorContextExtras::new(ctxd).await)).await;
    }
    self.get_extras().await.as_ref().unwrap().clone()
  }

  async fn receive_with_context(&mut self) -> ContextHandle {
    if self.get_props().await.get_context_decorator_chain().is_some() {
      let ctx_extras = self.ensure_extras().await;
      ctx_extras.get_context().await.clone()
    } else {
      ContextHandle::new(self.clone())
    }
  }

  async fn default_receive(&mut self) -> Result<(), ActorError> {
    let message = self.get_message().await.expect("Failed to retrieve message");

    if message.as_any().is::<PoisonPill>() {
      let me = self.get_self().await.unwrap();
      self.stop(&me).await;
      Ok(())
    } else {
      let context = self.receive_with_context().await;
      let actor_opt = self.get_actor().await;
      let actor = actor_opt.as_ref().unwrap();

      let result = actor.handle(context.clone()).await;

      if result.is_ok() && message.as_any().is::<AutoRespondHandle>() {
        let auto_respond = message
          .as_any()
          .downcast_ref::<AutoRespondHandle>()
          .expect("Failed to downcast to AutoRespondWrapper")
          .clone();
        let res = auto_respond.get_auto_response(context);
        self.respond(res).await
      }

      result
    }
  }

  async fn incarnate_actor(&mut self) {
    {
      let mut inner_mg = self.inner.lock().await;
      match &inner_mg.state {
        Some(state) => {
          state.store(State::Alive as u8, Ordering::SeqCst);
        }
        None => {
          inner_mg.state = Some(Arc::new(AtomicU8::new(State::Alive as u8)));
        }
      }
    }
    let ch = ContextHandle::new(self.clone());
    let actor = self.get_props().await.get_producer().run(ch).await;
    self.set_actor(Some(actor)).await;

    // TODO: metrics
  }

  async fn get_receiver_middleware_chain(&self) -> Option<ReceiverFunc> {
    let mg = self.inner.lock().await;
    mg.props.get_receiver_middleware_chain().clone()
  }

  async fn get_sender_middleware_chain(&self) -> Option<SenderFunc> {
    let mg = self.inner.lock().await;
    mg.props.get_sender_middleware_chain().clone()
  }

  pub async fn send_user_message(&self, pid: ExtendedPid, message: MessageHandle) {
    match self.get_sender_middleware_chain().await {
      Some(chain) => {
        let mut cloned = self.clone();
        let context = cloned.ensure_extras().await.get_sender_context().await;
        chain.run(context, pid, MessageEnvelope::new(message)).await;
      }
      _ => {
        pid.send_user_message(self.get_actor_system().await, message).await;
      }
    }
  }

  async fn set_message_or_envelope(&mut self, message: MessageHandle) {
    let mut inner_mg = self.inner.lock().await;
    let message_or_envelope = &mut inner_mg.message_or_envelope;
    match message_or_envelope {
      None => {
        *message_or_envelope = Some(MessageOrEnvelope::of_message(message));
      }
      Some(value) => {
        *value = MessageOrEnvelope::of_message(message);
      }
    }
  }

  async fn reset_message_or_envelope(&mut self) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.message_or_envelope = None;
  }

  async fn process_message(&mut self, message: MessageHandle) -> Result<(), ActorError> {
    let props = self.get_props().await;
    let has_receiver_middleware_chain = props.get_receiver_middleware_chain().is_some();
    let has_context_decorator_chain = props.get_context_decorator_chain().is_some();

    if has_context_decorator_chain || has_receiver_middleware_chain {
      let extras = self.ensure_extras().await;
      let mut receiver_context = extras.get_receiver_context().await;
      let message_envelope = MessageEnvelope::new(message);
      if has_receiver_middleware_chain {
        let chain = props.get_receiver_middleware_chain().unwrap();
        chain.run(receiver_context, message_envelope).await
      } else if has_context_decorator_chain {
        receiver_context.receive(message_envelope).await
      } else {
        panic!("No middleware chain or context decorator chain found")
      }
    } else {
      self.set_message_or_envelope(message).await;
      let result = self.default_receive().await;
      self.reset_message_or_envelope().await;
      result
    }
  }

  async fn restart(&mut self) {
    self.incarnate_actor().await;
    self
      .get_self()
      .await
      .unwrap()
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(MailboxMessage::ResumeMailbox),
      )
      .await;
    let result = self
      .invoke_user_message(MessageHandle::new(SystemMessage::Started(Started {})))
      .await;
    if result.is_err() {
      P_LOG.error("Failed to handle Started message", vec![]).await;
    }

    if let Some(extras) = self.get_extras().await {
      while !extras.get_stash().await.is_empty().await {
        let message = extras.get_stash().await.pop().await;
        let result = self.invoke_user_message(message.unwrap()).await;
        if result.is_err() {
          P_LOG.error("Failed to handle stashed message", vec![]).await;
        }
      }
    }
  }

  async fn finalize_stop(&mut self) {
    self
      .get_actor_system()
      .await
      .get_process_registry()
      .await
      .remove_process(&self.get_self().await.unwrap());
    let result = self
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::Stopped(Stopped {})))
      .await;
    if result.is_err() {
      P_LOG.error("Failed to handle Stopped message", vec![]).await;
    }
    let other_stopped = MessageHandle::new(Terminated {
      who: self.get_self().await.map(|x| x.inner),
      why: 0,
    });
    if let Some(extras) = self.get_extras().await {
      let watchers = extras.get_watchers().await;
      for watcher in watchers.to_vec().await {
        watcher
          .send_system_message(self.get_actor_system().await, other_stopped.clone())
          .await;
      }
      if let Some(parent) = self.get_parent().await {
        parent
          .send_system_message(self.get_actor_system().await, other_stopped)
          .await;
      }
    }
  }

  async fn stop_all_children(&mut self) {
    match self.get_extras().await {
      Some(extras) => {
        let children = extras.get_children().await;
        for child in children.to_vec().await {
          self.stop(&child).await;
        }
      }
      _ => {}
    }
  }

  async fn try_restart_or_terminate(&mut self) {
    match self.get_extras().await {
      Some(extras) if extras.get_children().await.is_empty().await => {
        let state = {
          let mg = self.inner.lock().await;
          let num = mg.state.as_ref().unwrap().load(Ordering::SeqCst);
          State::try_from(num).unwrap()
        };
        match state {
          State::Restarting => {
            self.cancel_receive_timeout().await;
            self.restart().await;
          }
          State::Stopping => {
            self.cancel_receive_timeout().await;
            self.finalize_stop().await;
          }
          _ => {}
        }
      }
      _ => {}
    }
  }

  async fn handle_stop(&mut self) {
    {
      let mg = self.inner.lock().await;
      if mg.state.as_ref().unwrap().load(Ordering::SeqCst) >= State::Stopping as u8 {
        return;
      }
      mg.state
        .as_ref()
        .unwrap()
        .store(State::Stopping as u8, Ordering::SeqCst);
    }
    let mh = MessageHandle::new(AutoReceiveMessage::Stopping(Stopping {}));
    let result = self.invoke_user_message(mh).await;
    if result.is_err() {
      P_LOG.error("Failed to handle Stopping message", vec![]).await;
    }
    self.stop_all_children().await;
    self.try_restart_or_terminate().await;
  }

  async fn handle_restart(&mut self) {
    {
      let mut mg = self.inner.lock().await;
      mg.state
        .as_mut()
        .unwrap()
        .store(State::Restarting as u8, Ordering::SeqCst);
    }
    let result = self
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::Restarting(Restarting {})))
      .await;
    if result.is_err() {
      P_LOG.error("Failed to handle Restarting message", vec![]).await;
    }
    self.stop_all_children().await;
    self.try_restart_or_terminate().await;

    // TODO: Metrics
  }

  async fn handle_watch(&mut self, watch: &Watch) {
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::new(watch.clone().watcher.unwrap(), self.get_actor_system().await);
    extras.get_watchers().await.add(pid).await;
  }

  async fn handle_unwatch(&mut self, unwatch: &Unwatch) {
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::new(unwatch.clone().watcher.unwrap(), self.get_actor_system().await);
    extras.get_watchers().await.remove(&pid).await;
  }

  async fn handle_failure(&mut self, f: &Failure) {
    let actor = self.get_actor().await.unwrap();
    if let Some(s) = actor.get_supervisor_strategy() {
      let sh = SupervisorHandle::new(self.clone());
      s.handle_failure(
        &self.get_actor_system().await,
        sh,
        f.who.clone(),
        f.restart_stats.clone(),
        f.reason.clone(),
        f.message.clone(),
      )
      .await;
    } else {
      let sh = SupervisorHandle::new(self.clone());
      self
        .get_props()
        .await
        .get_supervisor_strategy()
        .handle_failure(
          &self.get_actor_system().await,
          sh,
          f.who.clone(),
          f.restart_stats.clone(),
          f.reason.clone(),
          f.message.clone(),
        )
        .await;
    }
  }

  async fn handle_terminated(&mut self, terminated: &Terminated) {
    if let Some(mut extras) = self.get_extras().await {
      let pid = ExtendedPid::new(terminated.clone().who.unwrap(), self.get_actor_system().await);
      extras.remove_child(&pid).await;
    }

    let result = self.invoke_user_message(MessageHandle::new(terminated.clone())).await;
    if result.is_err() {
      P_LOG.error("Failed to handle Terminated message", vec![]).await;
    }
    self.try_restart_or_terminate().await;
  }

  async fn handle_root_failure(&self, failure: &Failure) {
    DEFAULT_SUPERVISION_STRATEGY
      .handle_failure(
        &self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        self.get_self().await.unwrap(),
        RestartStatistics::new(),
        failure.reason.clone(),
        failure.message.clone(),
      )
      .await;
  }
}

#[async_trait]
impl InfoPart for ActorContext {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.inner.lock().await;
    mg.parent.clone()
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.inner.lock().await;
    mg.self_pid.clone()
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.inner.lock().await;
    mg.self_pid = Some(pid);
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor.clone()
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor_system.clone()
  }
}

#[async_trait]
impl BasePart for ActorContext {
  async fn get_receive_timeout(&self) -> tokio::time::Duration {
    let inner_mg = self.inner.lock().await;
    inner_mg
      .receive_timeout
      .as_ref()
      .expect("Failed to retrieve receive_timeout")
      .clone()
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    self
      .get_extras()
      .await
      .as_ref()
      .expect("Failed to retrieve extras")
      .get_children()
      .await
      .to_vec()
      .await
  }

  async fn respond(&self, response: ResponseHandle) {
    let mh = MessageHandle::new(response);
    if self.get_sender().await.is_none() {
      self
        .get_actor_system()
        .await
        .get_dead_letter()
        .await
        .send_user_message(None, mh)
        .await;
    } else {
      let mut cloned = self.clone();
      let pid = self.get_sender().await.unwrap();
      cloned.send(pid, mh).await
    }
  }

  async fn stash(&mut self) {
    let extra = self.ensure_extras().await;
    let mut stash = extra.get_stash().await;
    stash.push(self.get_message().await.unwrap()).await;
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self().await.unwrap().inner;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(Watch { watcher: Some(id) }),
      )
      .await;
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self().await.unwrap().inner;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(Unwatch { watcher: Some(id) }),
      )
      .await;
  }

  async fn set_receive_timeout(&mut self, d: &tokio::time::Duration) {
    if d.as_nanos() == 0 {
      panic!("Duration must be greater than zero");
    }
    {
      let mg = self.inner.lock().await;
      if Some(*d) == mg.receive_timeout {
        return;
      }
    }

    let d = if *d < tokio::time::Duration::from_millis(1) {
      tokio::time::Duration::from_secs(0)
    } else {
      *d
    };
    {
      let mut mg = self.inner.lock().await;
      mg.receive_timeout = Some(d.clone());
    }

    self.ensure_extras().await;

    if d > tokio::time::Duration::from_secs(0) {
      let context = Arc::new(Mutex::new(self.clone()));
      self
        .get_extras()
        .await
        .as_mut()
        .unwrap()
        .init_or_reset_receive_timeout_timer(d, context)
        .await;
    }
  }

  async fn cancel_receive_timeout(&mut self) {
    if let Some(extras) = self.get_extras().await {
      if extras.get_receive_timeout_timer().await.is_some() {
        extras.kill_receive_timeout_timer().await;
      }
    }
  }

  async fn forward(&self, pid: &ExtendedPid) {
    let mg = self.inner.lock().await;
    if let Some(moe) = &mg.message_or_envelope {
      let message = moe.get_value();
      if let Some(sm) = message.as_any().downcast_ref::<SystemMessage>() {
        panic!("SystemMessage cannot be forwarded: {:?}", sm);
      } else {
        let sm = MessageHandle::new(moe.clone());
        pid.send_user_message(self.get_actor_system().await, sm).await;
      }
    }
  }

  async fn reenter_after(
    &self,
    _: &Future,
    _: Box<
      dyn FnOnce(Result<Box<dyn Any + Send>, Box<dyn Error + Send + Sync>>) -> BoxFuture<'static, ()> + Send + 'static,
    >,
  ) {
    todo!()
  }
}

#[async_trait]
impl MessagePart for ActorContext {
  async fn get_message(&self) -> Option<MessageHandle> {
    let inner_mg = self.inner.lock().await;
    let message_or_envelope = inner_mg.message_or_envelope.as_ref().unwrap().clone();
    Some(message_or_envelope.get_value())
  }

  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    let inner_mg = self.inner.lock().await;
    let message_or_envelope = inner_mg.message_or_envelope.as_ref().unwrap().clone();
    let message_headers = message_or_envelope.get_envelope().unwrap().get_headers().unwrap();
    ReadonlyMessageHeadersHandle::new(message_headers)
  }
}

#[async_trait]
impl SenderPart for ActorContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let inner_mg = self.inner.lock().await;
    let message_or_envelope = inner_mg
      .message_or_envelope
      .as_ref()
      .expect("Failed to retrieve message_or_envelope");
    message_or_envelope.get_sender().clone()
  }

  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle) {
    self.send_user_message(pid, message).await;
  }

  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let env = MessageEnvelope::new(message).with_sender(self.get_self().await.unwrap());
    let message_handle = MessageHandle::new(env);
    self.send_user_message(pid, message_handle).await;
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid) {
    let env = MessageEnvelope::new(message).with_sender(sender);
    let message_handle = MessageHandle::new(env);
    self.send_user_message(pid, message_handle).await;
  }

  async fn request_future(&self, _: ExtendedPid, _: MessageHandle, _: &tokio::time::Duration) -> Future {
    todo!()
  }
}

#[async_trait]
impl ReceiverPart for ActorContext {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    {
      let mut inner_mg = self.inner.lock().await;
      inner_mg.message_or_envelope = Some(MessageOrEnvelope::of_envelope(envelope));
    }
    let result = self.default_receive().await;
    {
      let mut inner_mg = self.inner.lock().await;
      inner_mg.message_or_envelope = None;
    }
    result
  }
}

#[async_trait]
impl SpawnerPart for ActorContext {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    match self
      .spawn_named(
        props,
        &self.get_actor_system().await.get_process_registry().await.next_id(),
      )
      .await
    {
      Ok(pid) => pid,
      Err(e) => panic!("Failed to spawn child: {:?}", e),
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
      Err(e) => panic!("Failed to spawn child: {:?}", e),
    }
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    if props.get_guardian_strategy().is_some() {
      panic!("props used to spawn child cannot have GuardianStrategy")
    }
    let id = format!("{}/{}", self.get_self().await.unwrap().id(), id);
    let result = match self.get_props().await.get_spawn_middleware_chain() {
      Some(chain) => {
        let sch = SpawnerContextHandle::new(self.clone());
        chain.run(self.get_actor_system().await, &id, props, sch).await
      }
      _ => {
        let sch = SpawnerContextHandle::new(self.clone());
        props.spawn(self.get_actor_system().await, &id, sch).await
      }
    };

    match result {
      Ok(pid) => {
        let extras = self.ensure_extras().await;
        let mut children = extras.get_children().await;
        children.add(pid.clone()).await;
        Ok(pid)
      }
      Err(e) => Err(e),
    }
  }
}

#[async_trait]
impl StopperPart for ActorContext {
  async fn stop(&mut self, pid: &ExtendedPid) {
    // TODO: MetricsProvider
    let inner_mg = self.inner.lock().await;
    pid.ref_process(inner_mg.actor_system.clone()).await.stop(&pid).await;
  }

  async fn stop_future(&mut self, _: &ExtendedPid) -> Future {
    todo!()
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let inner_mg = self.inner.lock().await;
    pid
      .send_user_message(
        inner_mg.actor_system.clone(),
        MessageHandle::new(AutoReceiveMessage::PoisonPill(PoisonPill {})),
      )
      .await;
  }

  async fn poison_future(&mut self, pid: &ExtendedPid) -> Future {
    todo!()
  }
}

#[async_trait]
impl ExtensionPart for ActorContext {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let extras = self.ensure_extras().await;
    extras.get_extensions().await.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let extras = self.ensure_extras().await;
    extras.get_extensions().await.set(ext).await;
  }
}

impl SenderContext for ActorContext {}
impl ReceiverContext for ActorContext {}

impl SpawnerContext for ActorContext {}

impl ExtensionContext for ActorContext {}

impl Context for ActorContext {}

#[async_trait]
impl MessageInvoker for ActorContext {
  async fn invoke_system_message(&mut self, message: MessageHandle) -> Result<(), ActorError> {
    let sm = message.as_any().downcast_ref::<SystemMessage>();
    if let Some(sm) = sm {
      match sm {
        SystemMessage::Started(_) => {
          let result = self.invoke_user_message(message.clone()).await;
          if result.is_err() {
            return result;
          }
        }
        SystemMessage::Stop(_) => {
          self.handle_stop().await;
        }
        SystemMessage::Restart(_) => {
          self.handle_restart().await;
        }
      }
    }
    if let Some(c) = message.as_any().downcast_ref::<Continuation>() {
      let mut mg = self.inner.lock().await;
      let moe = MessageOrEnvelope::of_message(c.message.clone());
      mg.message_or_envelope = Some(moe);
      (c.f)().await;
      mg.message_or_envelope = None;
    }
    if let Some(w) = message.as_any().downcast_ref::<Watch>() {
      self.handle_watch(w).await;
    }
    if let Some(uw) = message.as_any().downcast_ref::<Unwatch>() {
      self.handle_unwatch(uw).await;
    }
    if let Some(f) = message.as_any().downcast_ref::<Failure>() {
      self.handle_failure(f).await;
    }
    if let Some(t) = message.as_any().downcast_ref::<Terminated>() {
      self.handle_terminated(t).await;
    }
    Ok(())
  }

  async fn invoke_user_message(&mut self, message: MessageHandle) -> Result<(), ActorError> {
    let state = {
      let inner_mg = self.inner.lock().await;
      inner_mg.state.clone()
    };
    if state.as_ref().as_ref().unwrap().load(Ordering::SeqCst) == State::Stopped as u8 {
      return Ok(());
    }
    let mut influence_timeout = true;

    let receive_timeout = {
      let inner_mg = self.inner.lock().await;
      inner_mg.receive_timeout.clone()
    };

    if receive_timeout.unwrap_or_else(|| tokio::time::Duration::from_millis(0)) > tokio::time::Duration::from_millis(0)
    {
      influence_timeout = !message
        .as_any()
        .downcast_ref::<NotInfluenceReceiveTimeoutHandle>()
        .is_some();
      if influence_timeout {
        let mg = self.get_extras().await;
        if let Some(extras) = mg {
          extras.stop_receive_timeout_timer().await;
        }
      }
    }

    let result = self.process_message(message).await;

    let receive_timeout = {
      let inner_mg = self.inner.lock().await;
      inner_mg.receive_timeout.clone()
    };

    let t = receive_timeout.unwrap_or_else(|| tokio::time::Duration::from_secs(0));
    if t > tokio::time::Duration::from_secs(0) && influence_timeout {
      if let Some(extras) = self.get_extras().await {
        extras.reset_receive_timeout_timer(t).await;
      }
    }

    result
  }

  async fn escalate_failure(&self, reason: ActorInnerError, message: MessageHandle) {
    todo!()
  }
}

#[async_trait]
impl Supervisor for ActorContext {
  async fn get_children(&self) -> Vec<ExtendedPid> {
    if self.get_extras().await.is_none() {
      return vec![];
    }

    self.get_extras().await.unwrap().get_children().await.to_vec().await
  }

  async fn escalate_failure(&mut self, reason: ActorInnerError, message: MessageHandle) {
    let self_pid = self.get_self().await.expect("Failed to retrieve self_pid");
    if self
      .get_actor_system()
      .await
      .get_config()
      .await
      .developer_supervision_logging
    {
      println!(
        "[Supervision] Actor: {}, failed with message: {}, exception: {}",
        self_pid, message, reason
      );
      P_LOG
        .error(
          &format!(
            "[Supervision] Actor: {}, failed with message: {}, exception: {}",
            self_pid, message, reason
          ),
          vec![],
        )
        .await;
    }

    let failure = Failure::new(
      self_pid,
      reason,
      self.ensure_extras().await.restart_stats().await,
      message,
    );

    self
      .get_self()
      .await
      .unwrap()
      .send_system_message(self.get_actor_system().await, MessageHandle::new(failure.clone()))
      .await;

    if self.get_parent().await.is_none() {
      self.handle_root_failure(&failure).await
    } else {
      self
        .get_parent()
        .await
        .unwrap()
        .send_system_message(self.get_actor_system().await, MessageHandle::new(failure))
        .await;
    }
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      pid
        .send_system_message(
          self.get_actor_system().await,
          MessageHandle::new(SystemMessage::Restart(Restart {})),
        )
        .await;
    }
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      pid
        .send_system_message(
          self.get_actor_system().await,
          MessageHandle::new(SystemMessage::Stop(Stop {})),
        )
        .await;
    }
  }

  async fn resume_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      pid
        .send_system_message(
          self.get_actor_system().await,
          MessageHandle::new(MailboxMessage::ResumeMailbox),
        )
        .await;
    }
  }
}
