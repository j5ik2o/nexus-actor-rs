use async_trait::async_trait;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::actor::actor::actor::Actor;
use crate::actor::actor::actor_error::ActorError;
use crate::actor::actor::actor_handle::ActorHandle;
use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::actor_producer::ActorProducer;
use crate::actor::actor::continuer::Continuer;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::props::Props;
use crate::actor::actor::receiver_middleware_chain::ReceiverMiddlewareChain;
use crate::actor::actor::sender_middleware_chain::SenderMiddlewareChain;
use crate::actor::actor::spawner::SpawnError;
use crate::actor::actor::{PoisonPill, Stop, Terminated, Unwatch, Watch};
use crate::actor::actor_system::ActorSystem;
use crate::actor::auto_respond::{AutoRespond, AutoResponsive};
use crate::actor::context::actor_context_extras::ActorContextExtras;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::spawner_context_handle::SpawnerContextHandle;
use crate::actor::context::state::State;
use crate::actor::context::{
  BasePart, Context, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::dispatch::message_invoker::MessageInvoker;
use crate::actor::future::FutureProcess;
use crate::actor::log::P_LOG;
use crate::actor::message::auto_receive_message::AutoReceiveMessage;
use crate::actor::message::continuation::Continuation;
use crate::actor::message::failure::Failure;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::message_or_envelope::{
  unwrap_envelope_header, unwrap_envelope_sender, wrap_envelope, MessageEnvelope,
};
use crate::actor::message::messages::{Restart, Restarting, Started, Stopped, Stopping};
use crate::actor::message::not_influence_receive_timeout::NotInfluenceReceiveTimeoutHandle;
use crate::actor::message::readonly_message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::message::receive_timeout::ReceiveTimeout;
use crate::actor::message::response::ResponseHandle;
use crate::actor::message::system_message::SystemMessage;
use crate::actor::process::Process;
use crate::actor::supervisor::supervisor_strategy::{
  Supervisor, SupervisorHandle, SupervisorStrategy, DEFAULT_SUPERVISION_STRATEGY,
};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};

#[derive(Debug, Clone)]
pub struct ActorContextInner {
  actor: Option<ActorHandle>,
  actor_system: ActorSystem,
  extras: Arc<Mutex<Option<ActorContextExtras>>>,
  props: Props,
  parent: Option<ExtendedPid>,
  self_pid: Option<ExtendedPid>,
  receive_timeout: Option<tokio::time::Duration>,
  producer: Option<ActorProducer>,
  message_or_envelope_opt: Arc<Mutex<Option<MessageHandle>>>,
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

static_assertions::assert_impl_all!(ActorContext: Send, Sync);

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
        message_or_envelope_opt: Arc::new(Mutex::new(None)),
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

  pub async fn receive_timeout_handler(&mut self) {
    if let Some(extras) = self.get_extras().await {
      if extras.get_receive_timeout_timer().await.is_some() {
        self.cancel_receive_timeout().await;
        self
          .send(
            self.get_self_opt().await.unwrap(),
            MessageHandle::new(ReceiveTimeout {}),
          )
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
    let message = self.get_message_opt().await.expect("Failed to retrieve message");
    tracing::debug!("ActorContext::default_receive: message = {:?}", message);
    if message.as_any().is::<PoisonPill>() {
      // tracing::debug!("PoisonPill received");
      let me = self.get_self_opt().await.unwrap();
      self.stop(&me).await;
      Ok(())
    } else {
      tracing::debug!("Received message: {:?}", message);
      let context = self.receive_with_context().await;
      let mut actor_opt = self.get_actor().await;
      let actor = actor_opt.as_mut().unwrap();

      let result = actor.handle(context.clone()).await;

      let me = message.as_any().downcast_ref::<MessageEnvelope>();
      let ar = message.as_any().downcast_ref::<AutoRespond>();
      let msg = match (me, ar) {
        (Some(me), _) => me.get_message().as_any().downcast_ref::<AutoRespond>().cloned(),
        (_, Some(ar)) => Some(ar.clone()),
        _ => None,
      };

      if let Some(auto_respond) = msg {
        let res = auto_respond.get_auto_response(context).await;
        tracing::debug!("auto_response: response = {:?}", res);
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

  async fn get_receiver_middleware_chain(&self) -> Option<ReceiverMiddlewareChain> {
    let mg = self.inner.lock().await;
    mg.props.get_receiver_middleware_chain().clone()
  }

  async fn get_sender_middleware_chain(&self) -> Option<SenderMiddlewareChain> {
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

  async fn get_message_or_envelop(&self) -> MessageHandle {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.lock().await;
    mg.clone().unwrap()
  }

  async fn set_message_or_envelope(&mut self, message: MessageHandle) {
    let inner_mg = self.inner.lock().await;
    let mut moe_opt = inner_mg.message_or_envelope_opt.lock().await;
    *moe_opt = Some(message);
  }

  async fn reset_message_or_envelope(&mut self) {
    let inner_mg = self.inner.lock().await;
    let mut moe_opt = inner_mg.message_or_envelope_opt.lock().await;
    *moe_opt = None;
  }

  async fn process_message(&mut self, message: MessageHandle) -> Result<(), ActorError> {
    tracing::debug!("ActorContext::process_message: start message = {:?}", message);
    let props = self.get_props().await;

    if props.get_receiver_middleware_chain().is_some() {
      // tracing::debug!("ActorContext::process_message: get_receiver_middleware_chain");
      let extras = self.ensure_extras().await;
      let receiver_context = extras.get_receiver_context().await;
      let message_envelope = wrap_envelope(message.clone());
      let chain = props.get_receiver_middleware_chain().unwrap();
      return chain.run(receiver_context, message_envelope).await;
    }

    if props.get_context_decorator_chain().is_some() {
      // tracing::debug!("ActorContext::process_message: get_context_decorator_chain");
      let extras = self.ensure_extras().await;
      let mut receiver_context = extras.get_receiver_context().await;
      let message_envelope = wrap_envelope(message.clone());
      return receiver_context.receive(message_envelope).await;
    }

    tracing::debug!(
      "ActorContext::process_message: default_receive: message = {:?}",
      message
    );
    self.set_message_or_envelope(message).await;
    let result = self.default_receive().await;
    self.reset_message_or_envelope().await;
    result
  }

  async fn restart(&mut self) -> Result<(), ActorError> {
    self.incarnate_actor().await;
    self
      .get_self_opt()
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
      return result;
    }

    if let Some(extras) = self.get_extras().await {
      while !extras.get_stash().await.is_empty().await {
        let msg = extras.get_stash().await.pop().await.unwrap();
        let result = self.invoke_user_message(msg).await;
        if result.is_err() {
          P_LOG.error("Failed to handle stashed message", vec![]).await;
          return result;
        }
      }
    }
    Ok(())
  }

  async fn finalize_stop(&mut self) -> Result<(), ActorError> {
    self
      .get_actor_system()
      .await
      .get_process_registry()
      .await
      .remove_process(&self.get_self_opt().await.unwrap());
    let msg = MessageHandle::new(AutoReceiveMessage::Stopped(Stopped {}));
    let result = self.invoke_user_message(msg).await;
    if result.is_err() {
      P_LOG.error("Failed to handle Stopped message", vec![]).await;
      return result;
    }
    let other_stopped = MessageHandle::new(Terminated {
      who: self.get_self_opt().await.map(|x| x.inner_pid),
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
    Ok(())
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

  async fn try_restart_or_terminate(&mut self) -> Result<(), ActorError> {
    // tracing::debug!("ActorContext::try_restart_or_terminate");
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
            let result = self.restart().await;
            if result.is_err() {
              P_LOG.error("Failed to restart actor", vec![]).await;
              return result;
            }
          }
          State::Stopping => {
            self.cancel_receive_timeout().await;
            let result = self.finalize_stop().await;
            if result.is_err() {
              P_LOG.error("Failed to finalize stop", vec![]).await;
              return result;
            }
          }
          _ => {}
        }
      }
      _ => {}
    }
    Ok(())
  }

  async fn handle_stop(&mut self) -> Result<(), ActorError> {
    tracing::debug!("ActorContext::handle_stop: start");
    {
      let mg = self.inner.lock().await;
      if mg.state.as_ref().unwrap().load(Ordering::SeqCst) >= State::Stopping as u8 {
        return Ok(());
      }
      mg.state
        .as_ref()
        .unwrap()
        .store(State::Stopping as u8, Ordering::SeqCst);
    }
    let msg = MessageHandle::new(AutoReceiveMessage::Stopping(Stopping {}));
    let result = self.invoke_user_message(msg).await;
    if result.is_err() {
      P_LOG.error("Failed to handle Stopping message", vec![]).await;
      return result;
    }
    self.stop_all_children().await;
    let result = self.try_restart_or_terminate().await;
    if result.is_err() {
      P_LOG.error("Failed to try_restart_or_terminate", vec![]).await;
      return result;
    }
    tracing::debug!("ActorContext::handle_stop: finished");
    Ok(())
  }

  async fn handle_restart(&mut self) -> Result<(), ActorError> {
    tracing::debug!("ActorContext::handle_restart");
    {
      let mut mg = self.inner.lock().await;
      mg.state
        .as_mut()
        .unwrap()
        .store(State::Restarting as u8, Ordering::SeqCst);
    }
    let msg = MessageHandle::new(AutoReceiveMessage::Restarting(Restarting {}));
    let result = self.invoke_user_message(msg).await;
    if result.is_err() {
      P_LOG.error("Failed to handle Restarting message", vec![]).await;
      return result;
    }
    self.stop_all_children().await;
    let result = self.try_restart_or_terminate().await;
    if result.is_err() {
      P_LOG.error("Failed to try_restart_or_terminate", vec![]).await;
      return result;
    }

    // TODO: Metrics
    Ok(())
  }

  async fn handle_watch(&mut self, watch: &Watch) {
    // tracing::debug!("ActorContext::handle_watch: {:?}", watch);
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::new(watch.clone().watcher.unwrap(), self.get_actor_system().await);
    extras.get_watchers().await.add(pid).await;
  }

  async fn handle_unwatch(&mut self, unwatch: &Unwatch) {
    // tracing::debug!("ActorContext::handle_unwatch: {:?}", unwatch);
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::new(unwatch.clone().watcher.unwrap(), self.get_actor_system().await);
    extras.get_watchers().await.remove(&pid).await;
  }

  async fn handle_child_failure(&mut self, f: &Failure) {
    let self_pid = self.get_self_opt().await.unwrap();
    tracing::debug!("ActorContext::handle_child_failure: start: self = {}", self_pid,);
    let actor = self.get_actor().await.unwrap();
    if let Some(s) = actor.get_supervisor_strategy().await {
      tracing::debug!("ActorContext::handle_child_failure: actor side get_supervisor_strategy");
      s.handle_child_failure(
        &self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        f.who.clone(),
        f.restart_stats.clone(),
        f.reason.clone(),
        f.message.clone(),
      )
      .await;
      return;
    }
    tracing::debug!("ActorContext::handle_child_failure: props side get_supervisor_strategy: DEFAULT");
    self
      .get_props()
      .await
      .get_supervisor_strategy()
      .handle_child_failure(
        &self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        f.who.clone(),
        f.restart_stats.clone(),
        f.reason.clone(),
        f.message.clone(),
      )
      .await;
    tracing::debug!("ActorContext::handle_child_failure: finished: self = {}", self_pid,);
  }

  async fn handle_terminated(&mut self, terminated: &Terminated) -> Result<(), ActorError> {
    // tracing::debug!("ActorContext::handle_terminated: {:?}", terminated);
    if let Some(mut extras) = self.get_extras().await {
      let pid = ExtendedPid::new(terminated.clone().who.unwrap(), self.get_actor_system().await);
      extras.remove_child(&pid).await;
    }

    let msg = MessageHandle::new(terminated.clone());
    let result = self.invoke_user_message(msg.clone()).await;
    if result.is_err() {
      P_LOG.error("Failed to handle Terminated message", vec![]).await;
      return result;
    }
    let result = self.try_restart_or_terminate().await;
    if result.is_err() {
      P_LOG.error("Failed to try_restart_or_terminate", vec![]).await;
      return result;
    }
    Ok(())
  }

  async fn handle_root_failure(&mut self, failure: &Failure) {
    tracing::debug!("ActorContext::handle_root_failure: start");
    DEFAULT_SUPERVISION_STRATEGY
      .handle_child_failure(
        &self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        self.get_self_opt().await.unwrap(),
        failure.restart_stats.clone(),
        failure.reason.clone(),
        failure.message.clone(),
      )
      .await;
    tracing::debug!("ActorContext::handle_root_failure: finished");
  }
}

#[async_trait]
impl InfoPart for ActorContext {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.inner.lock().await;
    mg.parent.clone()
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
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
    // tracing::debug!("ActorContext::respond: response = {:?}", response);
    let mh = MessageHandle::new(response);
    // tracing::debug!("ActorContext::respond: mh = {:?}", mh);
    let sender = self.get_sender().await;
    // tracing::debug!("ActorContext::respond: sender = {:?}", sender);
    if sender.is_none() {
      self
        .get_actor_system()
        .await
        .get_dead_letter()
        .await
        .send_user_message(None, mh)
        .await;
    } else {
      let mut cloned = self.clone();
      let pid = self.get_sender().await;
      // tracing::debug!("ActorContext::respond: pid = {:?}", pid);
      cloned.send(pid.unwrap(), mh).await
    }
  }

  async fn stash(&mut self) {
    let extra = self.ensure_extras().await;
    let mut stash = extra.get_stash().await;
    stash.push(self.get_message_opt().await.unwrap()).await;
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self_opt().await.unwrap().inner_pid;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(Watch { watcher: Some(id) }),
      )
      .await;
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self_opt().await.unwrap().inner_pid;
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
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.lock().await;
    if let Some(message_or_envelope) = &*mg {
      if let Some(sm) = message_or_envelope.as_any().downcast_ref::<SystemMessage>() {
        panic!("SystemMessage cannot be forwarded: {:?}", sm);
      } else {
        pid
          .send_user_message(self.get_actor_system().await, message_or_envelope.clone())
          .await;
      }
    }
  }

  async fn reenter_after(&self, future: crate::actor::future::Future, continuer: Continuer) {
    let message = self.get_message_or_envelop().await;
    let system = self.get_actor_system().await;
    let self_ref = self.get_self_opt().await.unwrap();

    future
      .continue_with(move |result_message, result_error| {
        let message = message.clone();
        let continuation = continuer.clone();
        let system = system.clone();
        let self_ref = self_ref.clone();

        async move {
          self_ref
            .send_system_message(
              system,
              MessageHandle::new(Continuation::new(message, move || {
                let continuation = continuation.clone();
                let result_message = result_message.clone();
                let result_error = result_error.clone();
                async move {
                  continuation.run(result_message, result_error).await;
                }
              })),
            )
            .await
        }
      })
      .await
  }
}

#[async_trait]
impl MessagePart for ActorContext {
  async fn get_message_opt(&self) -> Option<MessageHandle> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.lock().await;
    let result = if let Some(message_or_envelope) = &*mg {
      Some(message_or_envelope.clone())
    } else {
      None
    };
    result
  }

  async fn get_message_header(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.lock().await;
    if let Some(moe) = &*mg {
      unwrap_envelope_header(moe.clone()).map(|x| ReadonlyMessageHeadersHandle::new(x))
    } else {
      None
    }
  }
}

#[async_trait]
impl SenderPart for ActorContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.lock().await;
    if let Some(message_or_envelope) = &*mg {
      unwrap_envelope_sender(message_or_envelope.clone())
    } else {
      None
    }
  }

  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle) {
    self.send_user_message(pid, message).await;
  }

  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let env = MessageEnvelope::new(message).with_sender(self.get_self_opt().await.unwrap());
    let message_handle = MessageHandle::new(env);
    self.send_user_message(pid, message_handle).await;
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid) {
    let env = MessageEnvelope::new(message).with_sender(sender);
    let message_handle = MessageHandle::new(env);
    self.send_user_message(pid, message_handle).await;
  }

  async fn request_future(
    &self,
    pid: ExtendedPid,
    message: MessageHandle,
    timeout: Duration,
  ) -> crate::actor::future::Future {
    let future_process = FutureProcess::new(self.get_actor_system().await, timeout.clone()).await;
    let future_pid = future_process.get_pid().await;
    let moe = MessageEnvelope::new(message).with_sender(future_pid);
    self.send_user_message(pid, MessageHandle::new(moe)).await;
    future_process.get_future().await
  }
}

#[async_trait]
impl ReceiverPart for ActorContext {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    self.set_message_or_envelope(MessageHandle::new(envelope)).await;
    let result = self.default_receive().await;
    self.reset_message_or_envelope().await;
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
    let id = format!("{}/{}", self.get_self_opt().await.unwrap().id(), id);
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

  async fn stop_future(&mut self, pid: &ExtendedPid) -> crate::actor::future::Future {
    let future_process = FutureProcess::new(self.get_actor_system().await, tokio::time::Duration::from_secs(10)).await;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(Watch {
          watcher: Some(future_process.get_pid().await.inner_pid),
        }),
      )
      .await;
    self.stop(pid).await;
    future_process.get_future().await
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

  async fn poison_future(&mut self, pid: &ExtendedPid) -> crate::actor::future::Future {
    let future_process = FutureProcess::new(self.get_actor_system().await, tokio::time::Duration::from_secs(10)).await;

    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(Watch {
          watcher: Some(future_process.get_pid().await.inner_pid),
        }),
      )
      .await;
    self.poison(pid).await;

    future_process.get_future().await
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
          let result = self.handle_stop().await;
          if result.is_err() {
            return result;
          }
        }
        SystemMessage::Restart(_) => {
          let result = self.handle_restart().await;
          if result.is_err() {
            return result;
          }
        }
      }
    }
    if let Some(c) = message.as_any().downcast_ref::<Continuation>() {
      self.set_message_or_envelope(c.message.clone()).await;
      (c.f).run().await;
      self.reset_message_or_envelope().await;
    }
    if let Some(w) = message.as_any().downcast_ref::<Watch>() {
      self.handle_watch(w).await;
    }
    if let Some(uw) = message.as_any().downcast_ref::<Unwatch>() {
      self.handle_unwatch(uw).await;
    }
    if let Some(f) = message.as_any().downcast_ref::<Failure>() {
      self.handle_child_failure(f).await;
    }
    if let Some(t) = message.as_any().downcast_ref::<Terminated>() {
      let result = self.handle_terminated(t).await;
      if result.is_err() {
        return result;
      }
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

  async fn escalate_failure(&mut self, reason: ActorInnerError, message: MessageHandle) {
    // TODO: Metrics

    let failure = Failure::new(
      self.get_self_opt().await.unwrap(),
      reason.clone(),
      self.ensure_extras().await.restart_stats().await,
      message.clone(),
    );

    let self_pid = self.get_self_opt().await.unwrap();

    self_pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(MailboxMessage::SuspendMailbox),
      )
      .await;

    if self.get_parent().await.is_none() {
      self.handle_root_failure(&failure).await;
    } else {
      let parent_pid = self.get_parent().await.unwrap();
      parent_pid
        .send_system_message(self.get_actor_system().await, MessageHandle::new(failure))
        .await;
    }
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

  async fn escalate_failure(&self, reason: ActorInnerError, message: MessageHandle) {
    let self_pid = self.get_self_opt().await.expect("Failed to retrieve self_pid");
    if self
      .get_actor_system()
      .await
      .get_config()
      .await
      .developer_supervision_logging
    {
      tracing::info!(
        "[Supervision] Actor: {}, failed with message: {}, exception: {}",
        self_pid,
        message,
        reason
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

    let mut cloned_self = self.clone();

    let failure = Failure::new(
      self_pid,
      reason,
      cloned_self.ensure_extras().await.restart_stats().await,
      message,
    );

    self
      .get_self_opt()
      .await
      .unwrap()
      .send_system_message(self.get_actor_system().await, MessageHandle::new(failure.clone()))
      .await;

    if self.get_parent().await.is_none() {
      cloned_self.handle_root_failure(&failure).await
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
