use std::any::Any;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::context::actor_context_extras::ActorContextExtras;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::receiver_context_handle::ReceiverContextHandle;
use crate::actor::context::sender_context_handle::SenderContextHandle;
use crate::actor::context::spawner_context_handle::SpawnerContextHandle;
use crate::actor::context::state::State;
use crate::actor::context::{
  BasePart, Context, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::core::Actor;
use crate::actor::core::ActorError;
use crate::actor::core::ActorHandle;
use crate::actor::core::Continuer;
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::core::Props;
use crate::actor::core::ReceiverMiddlewareChain;
use crate::actor::core::SenderMiddlewareChain;
use crate::actor::core::SpawnError;
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
use crate::actor::process::future::ActorFutureProcess;
use crate::actor::process::Process;
use crate::actor::supervisor::{Supervisor, SupervisorHandle, SupervisorStrategy, DEFAULT_SUPERVISION_STRATEGY};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use crate::generated::actor::{PoisonPill, Terminated, Unwatch, Watch};

use crate::actor::process::actor_future::ActorFuture;
use crate::metrics::ActorMetrics;
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Instant;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct ActorContextInner {
  actor: Option<ActorHandle>,
}

#[derive(Debug, Clone)]
pub struct ActorContext {
  inner: Arc<Mutex<ActorContextInner>>,
  extras: Arc<RwLock<Option<ActorContextExtras>>>,
  message_or_envelope_opt: Arc<RwLock<Option<MessageHandle>>>,
  state: Arc<AtomicU8>,
  receive_timeout: Arc<RwLock<Option<Duration>>>,
  props: Props,
  actor_system: WeakActorSystem,
  parent: Option<ExtendedPid>,
  self_pid: Arc<RwLock<Option<ExtendedPid>>>,
}

#[derive(Debug)]
pub struct ContextBorrow<'a> {
  actor_system: ActorSystem,
  props: &'a Props,
  parent: Option<&'a ExtendedPid>,
  self_pid: Option<ExtendedPid>,
  actor: Option<ActorHandle>,
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
    let extras = Arc::new(RwLock::new(None));
    let message_or_envelope_opt = Arc::new(RwLock::new(None));
    let state = Arc::new(AtomicU8::new(State::Alive as u8));
    let receive_timeout = Arc::new(RwLock::new(None));
    let self_pid = Arc::new(RwLock::new(None));
    let mut ctx = ActorContext {
      inner: Arc::new(Mutex::new(ActorContextInner { actor: None })),
      extras,
      message_or_envelope_opt,
      state,
      receive_timeout,
      props,
      actor_system: actor_system.downgrade(),
      parent,
      self_pid,
    };
    ctx.incarnate_actor().await;
    ctx
  }

  fn extras_cell(&self) -> Arc<RwLock<Option<ActorContextExtras>>> {
    self.extras.clone()
  }

  fn message_cell(&self) -> Arc<RwLock<Option<MessageHandle>>> {
    self.message_or_envelope_opt.clone()
  }

  fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before ActorContext")
  }

  pub fn props_ref(&self) -> &Props {
    &self.props
  }

  pub fn parent_ref(&self) -> Option<&ExtendedPid> {
    self.parent.as_ref()
  }

  pub async fn borrow(&self) -> ContextBorrow<'_> {
    let actor = {
      let inner = self.inner.lock().await;
      inner.actor.clone()
    };
    let self_pid = self.self_pid.read().await.clone();
    ContextBorrow {
      actor_system: self.actor_system(),
      props: &self.props,
      parent: self.parent.as_ref(),
      self_pid,
      actor,
    }
  }

  pub async fn snapshot(&self) -> ContextBorrow<'_> {
    self.borrow().await
  }

  async fn get_extras(&self) -> Option<ActorContextExtras> {
    let extras_cell = self.extras_cell();
    let extras_guard = extras_cell.read().await;
    let extras = extras_guard.clone();
    extras
  }

  async fn set_extras(&mut self, extras: Option<ActorContextExtras>) {
    let extras_cell = self.extras_cell();
    *extras_cell.write().await = extras;
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
    if let Some(existing) = self.get_extras().await {
      return existing;
    }

    let context = self.prepare_context_handle().await;
    let extras = ActorContextExtras::new(context).await;

    let extras_cell = self.extras_cell();
    let mut guard = extras_cell.write().await;
    if let Some(existing) = guard.as_ref() {
      return existing.clone();
    }
    *guard = Some(extras.clone());
    extras
  }

  async fn prepare_context_handle(&mut self) -> ContextHandle {
    let ctxd = self.clone();
    if let Some(decorator) = self.props_ref().get_context_decorator_chain() {
      decorator.run(ContextHandle::new(ctxd)).await
    } else {
      ContextHandle::new(ctxd)
    }
  }

  async fn receive_with_context(&mut self) -> ContextHandle {
    if self.props_ref().get_context_decorator_chain().is_some() {
      let ctx_extras = self.ensure_extras().await;
      if let Some(handle) = ctx_extras.get_context().await {
        handle
      } else {
        let refreshed = self.prepare_context_handle().await;
        ctx_extras.set_context(refreshed.clone()).await;
        refreshed
      }
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
      let borrow = self.borrow().await;
      let mut actor = borrow
        .into_actor()
        .expect("Actor is not initialized before default_receive");

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
    self.state.store(State::Alive as u8, Ordering::SeqCst);
    let ch = ContextHandle::new(self.clone());
    let actor = self.props_ref().get_producer().run(ch).await;
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
    self.props.get_receiver_middleware_chain().clone()
  }

  async fn get_sender_middleware_chain(&self) -> Option<SenderMiddlewareChain> {
    self.props.get_sender_middleware_chain().clone()
  }

  pub async fn send_user_message(&self, pid: ExtendedPid, message_handle: MessageHandle) {
    match self.get_sender_middleware_chain().await {
      Some(chain) => {
        let mut cloned = self.clone();
        let extras = cloned.ensure_extras().await;
        let sender_context = if let Some(context) = extras.get_sender_context().await {
          context
        } else {
          let refreshed = cloned.prepare_context_handle().await;
          extras.set_context(refreshed.clone()).await;
          SenderContextHandle::new(refreshed)
        };
        chain
          .run(sender_context, pid, MessageEnvelope::new(message_handle))
          .await;
      }
      _ => {
        pid.send_user_message(self.actor_system(), message_handle).await;
      }
    }
  }

  async fn get_message_or_envelop(&self) -> MessageHandle {
    let message_cell = self.message_cell();
    let message_guard = message_cell.read().await;
    let message = message_guard.clone().unwrap();
    message
  }

  async fn set_message_or_envelope(&mut self, message_handle: MessageHandle) {
    let message_cell = self.message_cell();
    let mut moe_opt = message_cell.write().await;
    *moe_opt = Some(message_handle);
  }

  async fn reset_message_or_envelope(&mut self) {
    let message_cell = self.message_cell();
    let mut moe_opt = message_cell.write().await;
    *moe_opt = None;
  }

  async fn process_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    if let Some(chain) = self.props_ref().get_receiver_middleware_chain() {
      let extras = self.ensure_extras().await;
      let receiver_context = if let Some(context) = extras.get_receiver_context().await {
        context
      } else {
        let refreshed = self.prepare_context_handle().await;
        extras.set_context(refreshed.clone()).await;
        ReceiverContextHandle::new(refreshed)
      };
      let message_envelope = wrap_envelope(message_handle.clone());
      return chain.run(receiver_context, message_envelope).await;
    }

    if self.props_ref().get_context_decorator_chain().is_some() {
      let extras = self.ensure_extras().await;
      let mut receiver_context = if let Some(context) = extras.get_receiver_context().await {
        context
      } else {
        let refreshed = self.prepare_context_handle().await;
        extras.set_context(refreshed.clone()).await;
        ReceiverContextHandle::new(refreshed)
      };
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
    let actor_system = self.actor_system();
    self
      .get_self_opt()
      .await
      .unwrap()
      .send_system_message(actor_system, MessageHandle::new(MailboxMessage::ResumeMailbox))
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
    let actor_system = self.actor_system();
    actor_system
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
      let actor_system = self.actor_system();
      for watcher in watchers.to_vec().await {
        ExtendedPid::new(watcher)
          .send_system_message(actor_system.clone(), other_stopped.clone())
          .await;
      }
      if let Some(parent) = self.get_parent().await {
        parent.send_system_message(actor_system, other_stopped).await;
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
        let state = State::try_from(self.state.load(Ordering::SeqCst)).unwrap();
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
    loop {
      let current = self.state.load(Ordering::SeqCst);
      if current >= State::Stopping as u8 {
        return Ok(());
      }
      if self
        .state
        .compare_exchange(current, State::Stopping as u8, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
      {
        break;
      }
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
    self.state.store(State::Restarting as u8, Ordering::SeqCst);
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
    let actor_system = self.actor_system();
    if let Some(s) = actor.get_supervisor_strategy().await {
      s.handle_child_failure(
        actor_system.clone(),
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
      .props_ref()
      .get_supervisor_strategy()
      .handle_child_failure(
        actor_system,
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
    let actor_system = self.actor_system();
    DEFAULT_SUPERVISION_STRATEGY
      .handle_child_failure(
        actor_system,
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
    let actor_system = self.actor_system();
    let config = actor_system.get_config().await;
    if config.is_metrics_enabled() {
      if let Some(extension_arc) = actor_system.get_extensions().await.get(*EXTENSION_ID).await {
        let mut extension = extension_arc.lock().await;
        if let Some(m) = extension.as_any_mut().downcast_mut::<Metrics>() {
          m.foreach(f).await;
        }
      }
    }
  }
}

impl<'a> ContextBorrow<'a> {
  pub fn actor_system(&self) -> &ActorSystem {
    &self.actor_system
  }

  pub fn props(&self) -> &'a Props {
    self.props
  }

  pub fn parent(&self) -> Option<&'a ExtendedPid> {
    self.parent
  }

  pub fn self_pid(&self) -> Option<&ExtendedPid> {
    self.self_pid.as_ref()
  }

  pub fn actor(&self) -> Option<&ActorHandle> {
    self.actor.as_ref()
  }

  pub fn into_actor(self) -> Option<ActorHandle> {
    self.actor
  }
}

#[async_trait]
impl InfoPart for ActorContext {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    self.parent.clone()
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    self.self_pid.read().await.clone()
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut guard = self.self_pid.write().await;
    *guard = Some(pid);
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let inner_mg = self.inner.lock().await;
    inner_mg.actor.clone()
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.actor_system()
  }
}

#[async_trait]
impl BasePart for ActorContext {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.receive_timeout.read().await.unwrap_or(Duration::ZERO)
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
        .actor_system()
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
        self.actor_system(),
        MessageHandle::new(SystemMessage::Watch(Watch { watcher: Some(id) })),
      )
      .await;
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let id = self.get_self_opt().await.unwrap().inner_pid;
    pid
      .send_system_message(
        self.actor_system(),
        MessageHandle::new(SystemMessage::Unwatch(Unwatch { watcher: Some(id) })),
      )
      .await;
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    let normalized = if *d < Duration::from_millis(1) {
      Duration::ZERO
    } else {
      *d
    };

    {
      let mut receive_timeout = self.receive_timeout.write().await;
      if receive_timeout.unwrap_or(Duration::ZERO) == normalized {
        return;
      }
      *receive_timeout = if normalized.is_zero() { None } else { Some(normalized) };
    }

    if normalized.is_zero() {
      self.cancel_receive_timeout().await;
      return;
    }

    let mut extra = self.ensure_extras().await;
    let context = Arc::new(RwLock::new(self.clone()));
    extra.init_or_reset_receive_timeout_timer(normalized, context).await;
  }

  async fn cancel_receive_timeout(&mut self) {
    {
      let mut receive_timeout = self.receive_timeout.write().await;
      *receive_timeout = None;
    }
    if let Some(extras) = self.get_extras().await {
      if extras.get_receive_timeout_timer().await.is_some() {
        extras.kill_receive_timeout_timer().await;
      }
    }
  }

  async fn forward(&self, pid: &ExtendedPid) {
    let message_cell = self.message_cell();
    let mg = message_cell.read().await;
    if let Some(message_or_envelope) = &*mg {
      if let Some(sm) = message_or_envelope.to_typed::<SystemMessage>() {
        panic!("SystemMessage cannot be forwarded: {:?}", sm);
      } else {
        pid
          .send_user_message(self.actor_system(), message_or_envelope.clone())
          .await;
      }
    }
  }

  async fn reenter_after(&self, future: ActorFuture, continuer: Continuer) {
    let message = self.get_message_or_envelop().await;
    let system = self.actor_system();
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
    let message_cell = self.message_cell();
    let mg = message_cell.read().await;
    if let Some(message_or_envelope) = &*mg {
      message_or_envelope.to_typed::<MessageEnvelope>()
    } else {
      None
    }
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    let message_cell = self.message_cell();
    let mg = message_cell.read().await;
    (*mg)
      .as_ref()
      .map(|message_or_envelope| unwrap_envelope_message(message_or_envelope.clone()))
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let message_cell = self.message_cell();
    let mg = message_cell.read().await;
    if let Some(moe) = &*mg {
      unwrap_envelope_header(moe.clone()).map(ReadonlyMessageHeadersHandle::new)
    } else {
      None
    }
  }
}

#[async_trait]
impl SenderPart for ActorContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let message_cell = self.message_cell();
    let mg = message_cell.read().await;
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

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    let actor_system = self.actor_system();
    let future_process = ActorFutureProcess::new(actor_system, timeout).await;
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
    let actor_system = self.actor_system();
    match self
      .spawn_named(props, &actor_system.get_process_registry().await.next_id())
      .await
    {
      Ok(pid) => pid,
      Err(e) => panic!("Failed to spawn child: {:?}", e),
    }
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let actor_system = self.actor_system();
    let next_id = actor_system.get_process_registry().await.next_id();
    match self.spawn_named(props, &format!("{}-{}", prefix, next_id)).await {
      Ok(pid) => pid,
      Err(e) => panic!("Failed to spawn child: {:?}", e),
    }
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    if props.get_guardian_strategy().is_some() {
      panic!("props used to spawn child cannot have GuardianStrategy")
    }
    let id = format!("{}/{}", self.get_self_opt().await.unwrap().id(), id);
    let actor_system = self.actor_system();
    let result = match self.props_ref().get_spawn_middleware_chain() {
      Some(chain) => {
        let sch = SpawnerContextHandle::new(self.clone());
        chain.run(actor_system.clone(), &id, props, sch).await
      }
      _ => {
        let sch = SpawnerContextHandle::new(self.clone());
        props.spawn(actor_system.clone(), &id, sch).await
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
    let actor_system = self.actor_system();
    pid.ref_process(actor_system).await.stop(pid).await;
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let actor_system = self.actor_system();
    let future_process = ActorFutureProcess::new(actor_system.clone(), timeout).await;
    pid
      .send_system_message(
        actor_system,
        MessageHandle::new(SystemMessage::Watch(Watch {
          watcher: Some(future_process.get_pid().await.inner_pid),
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
      .await;
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let actor_system = self.actor_system();
    let future_process = ActorFutureProcess::new(actor_system.clone(), timeout).await;

    pid
      .send_system_message(
        actor_system,
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
          result?;
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
    if self.state.load(Ordering::SeqCst) == State::Stopped as u8 {
      return Ok(());
    }
    let mut influence_timeout = true;

    let receive_timeout = { self.receive_timeout.read().await.clone() };

    if receive_timeout.unwrap_or_else(|| Duration::from_millis(0)) > Duration::from_millis(0) {
      influence_timeout = message_handle.to_typed::<NotInfluenceReceiveTimeoutHandle>().is_none();
      if influence_timeout {
        let mg = self.get_extras().await;
        if let Some(extras) = mg {
          extras.stop_receive_timeout_timer().await;
        }
      }
    }

    let actor_system = self.actor_system();
    let config = actor_system.get_config().await;
    let result = if config.metrics_provider.is_some() {
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

    let receive_timeout = { self.receive_timeout.read().await.clone() };

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

    let actor_system = self.actor_system();
    self_pid
      .send_system_message(actor_system.clone(), MessageHandle::new(MailboxMessage::SuspendMailbox))
      .await;

    if self.get_parent().await.is_none() {
      self.handle_root_failure(&failure).await;
    } else {
      let parent_pid = self.get_parent().await.unwrap();
      parent_pid
        .send_system_message(actor_system, MessageHandle::new(failure))
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
    let actor_system = self.actor_system();
    if actor_system.get_config().await.developer_supervision_logging {
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
      .send_system_message(actor_system.clone(), MessageHandle::new(failure.clone()))
      .await;

    if self.get_parent().await.is_none() {
      cloned_self.handle_root_failure(&failure).await
    } else {
      self
        .get_parent()
        .await
        .unwrap()
        .send_system_message(actor_system, MessageHandle::new(failure))
        .await;
    }
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    let actor_system = self.actor_system();
    for pid in pids {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(SystemMessage::Restart))
        .await;
    }
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    let actor_system = self.actor_system();
    for pid in pids {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(SystemMessage::Stop))
        .await;
    }
  }

  async fn resume_children(&self, pids: &[ExtendedPid]) {
    let actor_system = self.actor_system();
    for pid in pids {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(MailboxMessage::ResumeMailbox))
        .await;
    }
  }
}
