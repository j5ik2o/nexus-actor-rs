use std::any::Any;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context_extras::ActorContextExtras;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::spawner_context_handle::SpawnerContextHandle;
use crate::actor::context::state::State;
use crate::actor::context::{
  BasePart, Context, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::core::Actor;
use crate::actor::core::ActorError;
use crate::actor::core::ActorHandle;
use crate::actor::core::ActorProducer;
use crate::actor::core::Continuer;
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::core::Props;
use crate::actor::core::ReceiverMiddlewareChain;
use crate::actor::core::SenderMiddlewareChain;
use crate::actor::core::SpawnError;
use crate::actor::dispatch::future::ActorFutureProcess;
use crate::actor::dispatch::MailboxMessage;
use crate::actor::dispatch::MessageInvoker;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::Continuation;
use crate::actor::message::Failure;
use crate::actor::message::MessageHandle;
use crate::actor::message::NotInfluenceReceiveTimeoutHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::ReceiveTimeout;
use crate::actor::message::ResponseHandle;
use crate::actor::message::SystemMessage;
use crate::actor::message::TerminateReason;
use crate::actor::message::{
  unwrap_envelope_header, unwrap_envelope_message, unwrap_envelope_sender, wrap_envelope, MessageEnvelope,
};
use crate::actor::message::{AutoRespond, AutoResponsive};
use crate::actor::metrics::metrics_impl::{Metrics, EXTENSION_ID};
use crate::actor::process::Process;
use crate::actor::supervisor::{Supervisor, SupervisorHandle, SupervisorStrategy, DEFAULT_SUPERVISION_STRATEGY};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use crate::generated::actor::{PoisonPill, Terminated, Unwatch, Watch};

use crate::metrics::ActorMetrics;
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Instant;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct ActorContextInner {
  actor: Option<ActorHandle>,
  actor_system: ActorSystem,
  extras: Arc<RwLock<Option<ActorContextExtras>>>,
  props: Props,
  parent: Option<ExtendedPid>,
  self_pid: Option<ExtendedPid>,
  receive_timeout: Option<Duration>,
  producer: Option<ActorProducer>,
  message_or_envelope_opt: Arc<RwLock<Option<MessageHandle>>>,
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
        extras: Arc::new(RwLock::new(None)),
        props,
        parent,
        self_pid: None,
        receive_timeout: None,
        producer: None,
        message_or_envelope_opt: Arc::new(RwLock::new(None)),
        state: None,
      })),
    };
    ctx.incarnate_actor().await;
    ctx
  }

  async fn get_extras(&self) -> Option<ActorContextExtras> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.extras.read().await;
    mg.clone()
  }

  async fn set_extras(&mut self, extras: Option<ActorContextExtras>) {
    let inner_mg = self.inner.lock().await;
    *inner_mg.extras.write().await = extras;
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
          .send(self.get_self_opt().await.unwrap(), MessageHandle::new(ReceiveTimeout))
          .await;
      }
    }
  }

  pub(crate) async fn ensure_extras(&mut self) -> ActorContextExtras {
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
    let message = self.get_message_handle_opt().await.expect("Failed to retrieve message");
    if message.to_typed::<PoisonPill>().is_some() {
      let me = self.get_self().await;
      self.stop(&me).await;
      Ok(())
    } else {
      let context = self.receive_with_context().await;
      let mut actor_opt = self.get_actor().await;

      let actor = actor_opt.as_mut().unwrap();

      let result = actor.handle(context.clone()).await;

      let me = message.to_typed::<MessageEnvelope>();
      let ar = message.to_typed::<AutoRespond>();
      let msg = match (me, ar) {
        (Some(me), _) => me.get_message_handle().to_typed::<AutoRespond>(),
        (_, Some(ar)) => Some(ar.clone()),
        _ => None,
      };

      if let Some(auto_respond) = msg {
        let res = auto_respond.get_auto_response(context).await;
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

    self
      .metrics_foreach(|am, _| {
        let am = am.clone();
        async move {
          am.increment_actor_spawn_count().await;
        }
      })
      .await;
  }

  async fn get_receiver_middleware_chain(&self) -> Option<ReceiverMiddlewareChain> {
    let mg = self.inner.lock().await;
    mg.props.get_receiver_middleware_chain().clone()
  }

  async fn get_sender_middleware_chain(&self) -> Option<SenderMiddlewareChain> {
    let mg = self.inner.lock().await;
    mg.props.get_sender_middleware_chain().clone()
  }

  pub async fn send_user_message(&self, pid: ExtendedPid, message_handle: MessageHandle) {
    match self.get_sender_middleware_chain().await {
      Some(chain) => {
        let mut cloned = self.clone();
        let context = cloned.ensure_extras().await.get_sender_context().await;
        chain.run(context, pid, MessageEnvelope::new(message_handle)).await;
      }
      _ => {
        pid
          .send_user_message(self.get_actor_system().await, message_handle)
          .await;
      }
    }
  }

  async fn get_message_or_envelop(&self) -> MessageHandle {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.read().await;
    mg.clone().unwrap()
  }

  async fn set_message_or_envelope(&mut self, message_handle: MessageHandle) {
    let inner_mg = self.inner.lock().await;
    let mut moe_opt = inner_mg.message_or_envelope_opt.write().await;
    *moe_opt = Some(message_handle);
  }

  async fn reset_message_or_envelope(&mut self) {
    let inner_mg = self.inner.lock().await;
    let mut moe_opt = inner_mg.message_or_envelope_opt.write().await;
    *moe_opt = None;
  }

  async fn process_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    let props = self.get_props().await;

    if props.get_receiver_middleware_chain().is_some() {
      let extras = self.ensure_extras().await;
      let receiver_context = extras.get_receiver_context().await;
      let message_envelope = wrap_envelope(message_handle.clone());
      let chain = props.get_receiver_middleware_chain().unwrap();
      return chain.run(receiver_context, message_envelope).await;
    }

    if props.get_context_decorator_chain().is_some() {
      let extras = self.ensure_extras().await;
      let mut receiver_context = extras.get_receiver_context().await;
      let message_envelope = wrap_envelope(message_handle.clone());
      return receiver_context.receive(message_envelope).await;
    }

    self.set_message_or_envelope(message_handle).await;
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
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::PostRestart))
      .await;
    if result.is_err() {
      tracing::error!("Failed to handle Restarted message");
      return result;
    }

    self.un_stash_all().await
  }

  async fn finalize_stop(&mut self) -> Result<(), ActorError> {
    self
      .get_actor_system()
      .await
      .get_process_registry()
      .await
      .remove_process(&self.get_self_opt().await.unwrap())
      .await;
    let result = self
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::PostStop))
      .await;
    if result.is_err() {
      tracing::error!("Failed to handle Stopped message");
      return result;
    }
    let other_stopped = MessageHandle::new(SystemMessage::Terminate(Terminated {
      who: self.get_self_opt().await.map(|x| x.inner_pid),
      why: TerminateReason::Stopped as i32,
    }));
    if let Some(extras) = self.get_extras().await {
      let watchers = extras.get_watchers().await;
      for watcher in watchers.to_vec().await {
        ExtendedPid::new(watcher)
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
    let extras = self.ensure_extras().await;
    let children = extras.get_children().await;
    for child in children.to_vec().await {
      let child = ExtendedPid::new(child);
      self.stop(&child).await;
    }
  }

  async fn try_restart_or_terminate(&mut self) -> Result<(), ActorError> {
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
              tracing::error!("Failed to restart actor");
              return result;
            }
          }
          State::Stopping => {
            self.cancel_receive_timeout().await;
            let result = self.finalize_stop().await;
            if result.is_err() {
              tracing::error!("Failed to finalize stop");
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

  async fn handle_start(&mut self) -> Result<(), ActorError> {
    self
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::PostStart))
      .await?;
    Ok(())
  }

  async fn handle_stop(&mut self) -> Result<(), ActorError> {
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
    let result = self
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::PreStop))
      .await;
    if result.is_err() {
      tracing::error!("Failed to handle Stopping message");
      return result;
    }
    self.stop_all_children().await;
    let result = self.try_restart_or_terminate().await;
    if result.is_err() {
      tracing::error!("Failed to try_restart_or_terminate");
      return result;
    }
    Ok(())
  }

  async fn handle_restart(&mut self) -> Result<(), ActorError> {
    {
      let mut mg = self.inner.lock().await;
      mg.state
        .as_mut()
        .unwrap()
        .store(State::Restarting as u8, Ordering::SeqCst);
    }
    let result = self
      .invoke_user_message(MessageHandle::new(AutoReceiveMessage::PreRestart))
      .await;
    if result.is_err() {
      tracing::error!("Failed to handle Restarting message");
      return result;
    }
    self.stop_all_children().await;
    let result = self.try_restart_or_terminate().await;
    if result.is_err() {
      tracing::error!("Failed to try_restart_or_terminate");
      return result;
    }

    self
      .metrics_foreach(|am, _| {
        let am = am.clone();
        async move { am.increment_actor_restarted_count().await }
      })
      .await;
    Ok(())
  }

  async fn handle_watch(&mut self, watch: &Watch) {
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::new(watch.clone().watcher.unwrap());
    extras.get_watchers().await.add(pid.inner_pid).await;
  }

  async fn handle_unwatch(&mut self, unwatch: &Unwatch) {
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::new(unwatch.clone().watcher.unwrap());
    extras.get_watchers().await.remove(&pid.inner_pid).await;
  }

  async fn handle_child_failure(&mut self, f: &Failure) {
    let mut actor = self.get_actor().await.unwrap();
    if let Some(s) = actor.get_supervisor_strategy().await {
      s.handle_child_failure(
        self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        f.who.clone(),
        f.restart_stats.clone(),
        f.reason.clone(),
        f.message_handle.clone(),
      )
      .await;
      return;
    }
    self
      .get_props()
      .await
      .get_supervisor_strategy()
      .handle_child_failure(
        self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        f.who.clone(),
        f.restart_stats.clone(),
        f.reason.clone(),
        f.message_handle.clone(),
      )
      .await;
  }

  async fn handle_terminated(&mut self, terminated: &Terminated) -> Result<(), ActorError> {
    if let Some(mut extras) = self.get_extras().await {
      let pid = ExtendedPid::new(terminated.clone().who.unwrap());
      extras.remove_child(&pid).await;
    }

    let msg = MessageHandle::new(AutoReceiveMessage::Terminated(terminated.clone()));
    let result = self.invoke_user_message(msg.clone()).await;
    if result.is_err() {
      tracing::error!("Failed to handle Terminated message");
      return result;
    }
    let result = self.try_restart_or_terminate().await;
    if result.is_err() {
      tracing::error!("Failed to try_restart_or_terminate");
      return result;
    }
    Ok(())
  }

  async fn handle_root_failure(&mut self, failure: &Failure) {
    DEFAULT_SUPERVISION_STRATEGY
      .handle_child_failure(
        self.get_actor_system().await,
        SupervisorHandle::new(self.clone()),
        self.get_self_opt().await.unwrap(),
        failure.restart_stats.clone(),
        failure.reason.clone(),
        failure.message_handle.clone(),
      )
      .await;
  }

  async fn metrics_foreach<F, Fut>(&self, f: F)
  where
    F: Fn(&ActorMetrics, &Metrics) -> Fut,
    Fut: std::future::Future<Output = ()>, {
    if self.get_actor_system().await.get_config().await.is_metrics_enabled() {
      if let Some(extension_arc) = self
        .get_actor_system()
        .await
        .get_extensions()
        .await
        .get(*EXTENSION_ID)
        .await
      {
        let mut extension = extension_arc.lock().await;
        if let Some(m) = extension.as_any_mut().downcast_mut::<Metrics>() {
          m.foreach(f).await;
        }
      }
    }
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
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    let inner_mg = self.inner.lock().await;
    *inner_mg
      .receive_timeout
      .as_ref()
      .expect("Failed to retrieve receive_timeout")
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
      .into_iter()
      .map(ExtendedPid::new)
      .collect()
  }

  async fn respond(&self, response: ResponseHandle) {
    let mh = MessageHandle::new(response);
    let sender = self.get_sender().await;
    if sender.is_none() {
      tracing::info!("ActorContext::respond: sender is none");
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
      tracing::info!("ActorContext::respond: pid = {:?}", pid);
      cloned.send(pid.unwrap(), mh).await
    }
  }

  async fn stash(&mut self) {
    let extra = self.ensure_extras().await;
    let mut stash = extra.get_stash().await;
    stash.push(self.get_message_handle().await).await;
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    if let Some(extras) = self.get_extras().await {
      while !extras.get_stash().await.is_empty().await {
        let msg = extras.get_stash().await.pop().await.unwrap();
        let result = self.invoke_user_message(msg).await;
        if result.is_err() {
          tracing::error!("Failed to handle stashed message");
          return result;
        }
      }
    }
    Ok(())
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self_opt().await.unwrap().inner_pid;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(SystemMessage::Watch(Watch { watcher: Some(id) })),
      )
      .await;
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self_opt().await.unwrap().inner_pid;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(SystemMessage::Unwatch(Unwatch { watcher: Some(id) })),
      )
      .await;
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    if d.as_nanos() == 0 {
      panic!("Duration must be greater than zero");
    }
    {
      let mg = self.inner.lock().await;
      if Some(*d) == mg.receive_timeout {
        return;
      }
    }

    let d = if *d < Duration::from_millis(1) {
      Duration::from_secs(0)
    } else {
      *d
    };
    {
      let mut mg = self.inner.lock().await;
      mg.receive_timeout = Some(d);
    }

    let mut extra = self.ensure_extras().await;

    if d > Duration::from_secs(0) {
      let context = Arc::new(RwLock::new(self.clone()));
      extra.init_or_reset_receive_timeout_timer(d, context).await;
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
    let mg = inner_mg.message_or_envelope_opt.read().await;
    if let Some(message_or_envelope) = &*mg {
      if let Some(sm) = message_or_envelope.to_typed::<SystemMessage>() {
        panic!("SystemMessage cannot be forwarded: {:?}", sm);
      } else {
        pid
          .send_user_message(self.get_actor_system().await, message_or_envelope.clone())
          .await;
      }
    }
  }

  async fn reenter_after(&self, future: crate::actor::dispatch::future::ActorFuture, continuer: Continuer) {
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
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.read().await;
    if let Some(message_or_envelope) = &*mg {
      message_or_envelope.to_typed::<MessageEnvelope>()
    } else {
      None
    }
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.read().await;
    if let Some(message_or_envelope) = &*mg {
      Some(unwrap_envelope_message(message_or_envelope.clone()))
    } else {
      None
    }
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let inner_mg = self.inner.lock().await;
    let mg = inner_mg.message_or_envelope_opt.read().await;
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
    let mg = inner_mg.message_or_envelope_opt.read().await;
    if let Some(message_or_envelope) = &*mg {
      unwrap_envelope_sender(message_or_envelope.clone())
    } else {
      None
    }
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    self.send_user_message(pid, message_handle).await;
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let env = MessageEnvelope::new(message_handle).with_sender(self.get_self_opt().await.unwrap());
    let message_handle = MessageHandle::new(env);
    self.send_user_message(pid, message_handle).await;
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    let env = MessageEnvelope::new(message_handle).with_sender(sender);
    let message_handle = MessageHandle::new(env);
    self.send_user_message(pid, message_handle).await;
  }

  async fn request_future(
    &self,
    pid: ExtendedPid,
    message_handle: MessageHandle,
    timeout: Duration,
  ) -> crate::actor::dispatch::future::ActorFuture {
    let future_process = ActorFutureProcess::new(self.get_actor_system().await, timeout.clone()).await;
    let future_pid = future_process.get_pid().await;
    let moe = MessageEnvelope::new(message_handle).with_sender(future_pid);
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
        children.add(pid.inner_pid.clone()).await;
        Ok(pid)
      }
      Err(e) => Err(e),
    }
  }
}

#[async_trait]
impl StopperPart for ActorContext {
  async fn stop(&mut self, pid: &ExtendedPid) {
    self
      .metrics_foreach(|am, _| {
        let am = am.clone();
        async move { am.increment_actor_stopped_count().await }
      })
      .await;
    let inner_mg = self.inner.lock().await;
    pid.ref_process(inner_mg.actor_system.clone()).await.stop(&pid).await;
  }

  async fn stop_future_with_timeout(
    &mut self,
    pid: &ExtendedPid,
    timeout: Duration,
  ) -> crate::actor::dispatch::future::ActorFuture {
    let future_process = ActorFutureProcess::new(self.get_actor_system().await, timeout).await;
    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_process.get_pid().await.inner_pid),
        })),
      )
      .await;
    self.stop(pid).await;
    future_process.get_future().await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let inner_mg = self.inner.lock().await;
    pid
      .send_user_message(inner_mg.actor_system.clone(), MessageHandle::new(PoisonPill {}))
      .await;
  }

  async fn poison_future_with_timeout(
    &mut self,
    pid: &ExtendedPid,
    timeout: Duration,
  ) -> crate::actor::dispatch::future::ActorFuture {
    let future_process = ActorFutureProcess::new(self.get_actor_system().await, timeout).await;

    pid
      .send_system_message(
        self.get_actor_system().await,
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_process.get_pid().await.inner_pid),
        })),
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
  async fn invoke_system_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    let sm = message_handle.to_typed::<SystemMessage>();
    if let Some(sm) = sm {
      match sm {
        SystemMessage::Start => {
          let result = self.handle_start().await;
          if result.is_err() {
            return result;
          }
        }
        SystemMessage::Stop => {
          self.handle_stop().await?;
        }
        SystemMessage::Restart => {
          self.handle_restart().await?;
        }
        SystemMessage::Watch(watch) => {
          self.handle_watch(&watch).await;
        }
        SystemMessage::Unwatch(unwatch) => {
          self.handle_unwatch(&unwatch).await;
        }
        SystemMessage::Terminate(t) => {
          self.handle_terminated(&t).await?;
        }
      }
    }
    if let Some(c) = message_handle.to_typed::<Continuation>() {
      self.set_message_or_envelope(c.message_handle.clone()).await;
      (c.f).run().await;
      self.reset_message_or_envelope().await;
    }
    if let Some(f) = message_handle.to_typed::<Failure>() {
      self.handle_child_failure(&f).await;
    }
    Ok(())
  }

  async fn invoke_user_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
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

    if receive_timeout.unwrap_or_else(|| Duration::from_millis(0)) > Duration::from_millis(0) {
      influence_timeout = !message_handle.to_typed::<NotInfluenceReceiveTimeoutHandle>().is_some();
      if influence_timeout {
        let mg = self.get_extras().await;
        if let Some(extras) = mg {
          extras.stop_receive_timeout_timer().await;
        }
      }
    }

    let result = if self
      .get_actor_system()
      .await
      .get_config()
      .await
      .metrics_provider
      .is_some()
    {
      let start = Instant::now();
      let result = self.process_message(message_handle).await;
      let duration = start.elapsed();
      self
        .metrics_foreach(|am, _| {
          let am = am.clone();
          async move {
            am.record_actor_message_receive_duration(duration.as_secs_f64()).await;
          }
        })
        .await;
      result
    } else {
      self.process_message(message_handle).await
    };

    let receive_timeout = {
      let inner_mg = self.inner.lock().await;
      inner_mg.receive_timeout.clone()
    };

    let t = receive_timeout.unwrap_or_else(|| Duration::from_secs(0));
    if t > Duration::from_secs(0) && influence_timeout {
      if let Some(extras) = self.get_extras().await {
        extras.reset_receive_timeout_timer(t).await;
      }
    }

    result
  }

  async fn escalate_failure(&mut self, reason: ErrorReason, message_handle: MessageHandle) {
    tracing::info!("[ACTOR] Recovering: reason = {:?}", reason.backtrace(),);

    self
      .metrics_foreach(|am, _| {
        let am = am.clone();
        async move { am.increment_actor_failure_count().await }
      })
      .await;

    let failure = Failure::new(
      self.get_self_opt().await.unwrap(),
      reason.clone(),
      self.ensure_extras().await.restart_stats().await,
      message_handle.clone(),
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
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    if self.get_extras().await.is_none() {
      return vec![];
    }
    self
      .get_extras()
      .await
      .unwrap()
      .get_children()
      .await
      .to_vec()
      .await
      .into_iter()
      .map(ExtendedPid::new)
      .collect()
  }

  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle) {
    let self_pid = self.get_self_opt().await.expect("Failed to retrieve self_pid");
    if self
      .get_actor_system()
      .await
      .get_config()
      .await
      .developer_supervision_logging
    {
      tracing::error!(
        "[Supervision] Actor: {}, failed with message: {}, exception: {}",
        self_pid,
        message_handle,
        reason
      );
    }

    self
      .metrics_foreach(|am, m| {
        let am = am.clone();
        let m = m.clone();
        async move {
          am.increment_actor_failure_count_with_opts(&m.common_labels(self).await)
            .await;
        }
      })
      .await;

    let mut cloned_self = self.clone();

    let failure = Failure::new(
      self_pid,
      reason,
      cloned_self.ensure_extras().await.restart_stats().await,
      message_handle,
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
          MessageHandle::new(SystemMessage::Restart),
        )
        .await;
    }
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      pid
        .send_system_message(self.get_actor_system().await, MessageHandle::new(SystemMessage::Stop))
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
