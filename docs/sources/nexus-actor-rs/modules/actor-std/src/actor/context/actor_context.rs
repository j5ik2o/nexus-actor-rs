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
  BasePart, Context, CoreSenderPart, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext,
  ReceiverPart, SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
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
use crate::actor::core_types::message_types::Message as _;
use crate::actor::dispatch::{
  MailboxMessage, MailboxQueueKind, MailboxQueueLatencyMetrics, MailboxSuspensionMetrics, MessageInvoker,
};
use crate::actor::message::Continuation;
use crate::actor::message::Failure;
use crate::actor::message::MessageHandle;
use crate::actor::message::NotInfluenceReceiveTimeoutHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::ReceiveTimeout;
use crate::actor::message::ResponseHandle;
use crate::actor::message::SystemMessage;
use crate::actor::message::TerminateReason;
use crate::actor::message::{wrap_envelope, MessageEnvelope};
use crate::actor::message::{AutoReceiveMessage, TerminatedMessage, UnwatchMessage, WatchMessage};
use crate::actor::message::{AutoRespond, AutoResponsive};
use crate::actor::metrics::metrics_impl::{MetricsRuntime, MetricsSink};
use crate::actor::process::future::ActorFutureProcess;
use crate::actor::process::Process;
use crate::actor::supervisor::{
  StdSupervisorAdapter, StdSupervisorContext, Supervisor, SupervisorHandle, DEFAULT_SUPERVISION_STRATEGY,
};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use crate::generated::actor::PoisonPill;
use arc_swap::ArcSwapOption;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::context::{CoreProps as CoreContextProps, CoreSupervisorStrategyHandle};

use crate::actor::process::actor_future::ActorFuture;
use async_trait::async_trait;
use tokio::sync::{OnceCell, RwLock};
use tokio::time::Instant;

#[cfg(test)]
mod tests;

#[derive(Debug)]
struct ActorContextShared {
  actor: OnceCell<Arc<ArcSwapOption<ActorHandle>>>,
}

impl Default for ActorContextShared {
  fn default() -> Self {
    let cell = OnceCell::new();
    let _ = cell.set(Arc::new(ArcSwapOption::from(None::<Arc<ActorHandle>>)));
    Self { actor: cell }
  }
}

impl ActorContextShared {
  fn swap(&self) -> &Arc<ArcSwapOption<ActorHandle>> {
    self.actor.get().expect("ActorContextShared actor swap not initialized")
  }
}

#[derive(Debug, Default)]
struct MailboxMetricsState {
  last_resume_events: u64,
  last_total_suspension_secs: f64,
  user_bucket_counts: Vec<u64>,
  system_bucket_counts: Vec<u64>,
  last_suspended_state: bool,
}

#[derive(Debug, Clone)]
pub struct ActorContext {
  shared: Arc<ActorContextShared>,
  extras: Arc<RwLock<Option<ActorContextExtras>>>,
  message_or_envelope: Arc<ArcSwapOption<MessageHandle>>,
  state: Arc<AtomicU8>,
  receive_timeout: Arc<RwLock<Option<Duration>>>,
  props: Props,
  core_props: CoreContextProps,
  actor_system: WeakActorSystem,
  parent: Option<ExtendedPid>,
  self_pid: Arc<OnceCell<Arc<ArcSwapOption<ExtendedPid>>>>,
  metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  metrics_sink: Arc<OnceCell<Arc<MetricsSink>>>,
  actor_type: Arc<OnceCell<Arc<str>>>,
  base_context_handle: Arc<OnceCell<ContextHandle>>,
  mailbox_metrics_state: Arc<RwLock<MailboxMetricsState>>,
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
    Arc::ptr_eq(&self.shared, &other.shared)
  }
}

impl Eq for ActorContext {}

impl std::hash::Hash for ActorContext {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.shared.as_ref() as *const ActorContextShared).hash(state);
  }
}

static_assertions::assert_impl_all!(ActorContext: Send, Sync);

impl ActorContext {
  fn actor_handle(&self) -> Option<ActorHandle> {
    self.shared.swap().load_full().map(|inner| (*inner).clone())
  }

  fn self_pid_swap(&self) -> &Arc<ArcSwapOption<ExtendedPid>> {
    self.self_pid.get().expect("ActorContext self_pid swap not initialized")
  }

  pub async fn new(actor_system: ActorSystem, props: Props, parent: Option<ExtendedPid>) -> Self {
    let extras = Arc::new(RwLock::new(None));
    let message_or_envelope = Arc::new(ArcSwapOption::from(None::<Arc<MessageHandle>>));
    let state = Arc::new(AtomicU8::new(State::Alive as u8));
    let receive_timeout = Arc::new(RwLock::new(None));
    let self_pid_cell = OnceCell::new();
    let initial_self = parent.as_ref().map(|pid| Arc::new(pid.clone()));
    let _ = self_pid_cell.set(Arc::new(ArcSwapOption::from(initial_self)));
    let metrics_runtime = actor_system.metrics_runtime_slot();
    let metrics_sink = Arc::new(OnceCell::new());
    let actor_type = Arc::new(OnceCell::new());
    let core_props = props.core_props();
    let mut ctx = ActorContext {
      shared: Arc::new(ActorContextShared::default()),
      extras,
      message_or_envelope,
      state,
      receive_timeout,
      props,
      core_props,
      actor_system: actor_system.downgrade(),
      parent,
      self_pid: Arc::new(self_pid_cell),
      metrics_runtime,
      metrics_sink,
      actor_type,
      base_context_handle: Arc::new(OnceCell::new()),
      mailbox_metrics_state: Arc::new(RwLock::new(MailboxMetricsState::default())),
    };
    if let Some(hint) = ctx.props.actor_type_hint() {
      let _ = ctx.actor_type.set(hint.clone());
      let _ = ctx.init_metrics_sink_with_type(Some(hint.as_ref()));
    }
    ctx.incarnate_actor().await;
    ctx
  }

  fn extras_cell(&self) -> Arc<RwLock<Option<ActorContextExtras>>> {
    self.extras.clone()
  }

  fn message_swap(&self) -> &Arc<ArcSwapOption<MessageHandle>> {
    &self.message_or_envelope
  }

  fn load_message_arc(&self) -> Option<Arc<MessageHandle>> {
    self.message_swap().load_full()
  }

  pub fn try_message_envelope(&self) -> Option<MessageEnvelope> {
    self
      .load_message_arc()
      .and_then(|handle| handle.as_ref().to_typed::<MessageEnvelope>())
  }

  pub fn try_message_handle(&self) -> Option<MessageHandle> {
    self
      .load_message_arc()
      .map(|handle| handle.as_ref().clone())
      .map(|handle| {
        if let Some(env) = handle.to_typed::<MessageEnvelope>() {
          env.get_message_handle()
        } else {
          handle
        }
      })
  }

  pub fn try_sender(&self) -> Option<ExtendedPid> {
    self.load_message_arc().and_then(|handle| {
      if let Some(env) = handle.as_ref().to_typed::<MessageEnvelope>() {
        env.get_sender()
      } else {
        None
      }
    })
  }

  pub fn try_message_header(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self
      .load_message_arc()
      .and_then(|handle| {
        handle
          .as_ref()
          .to_typed::<MessageEnvelope>()
          .and_then(|env| env.get_header())
      })
      .map(ReadonlyMessageHeadersHandle::new)
  }

  pub(crate) fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before ActorContext")
  }

  fn base_context_handle(&self) -> ContextHandle {
    if let Some(handle) = self.base_context_handle.get() {
      return handle.clone();
    }
    let handle = ContextHandle::new(self.clone());
    let _ = self.base_context_handle.set(handle.clone());
    handle
  }

  pub(crate) fn context_handle(&self) -> ContextHandle {
    self.base_context_handle()
  }

  pub fn props_ref(&self) -> &Props {
    &self.props
  }

  pub fn core_props(&self) -> &CoreContextProps {
    &self.core_props
  }

  pub fn parent_ref(&self) -> Option<&ExtendedPid> {
    self.parent.as_ref()
  }

  pub fn borrow(&self) -> ContextBorrow<'_> {
    let actor = self.actor_handle();
    let self_pid = self.self_pid_swap().load_full().map(|inner| (*inner).clone());
    ContextBorrow {
      actor_system: self.actor_system(),
      props: &self.props,
      parent: self.parent.as_ref(),
      self_pid,
      actor,
    }
  }

  pub fn snapshot(&self) -> ContextBorrow<'_> {
    self.borrow()
  }

  pub fn with_typed_borrow<M, R, F>(&self, f: F) -> R
  where
    M: crate::actor::message::Message,
    F: for<'a> FnOnce(crate::actor::context::TypedContextBorrow<'a, M>) -> R, {
    let borrow = self.borrow();
    let context_handle = self.context_handle();
    let view = crate::actor::context::TypedContextBorrow::new(self, context_handle, borrow);
    f(view)
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
    self.actor_handle()
  }

  fn set_actor(&self, actor: ActorHandle) {
    self.shared.swap().store(Some(Arc::new(actor)));
  }

  fn init_metrics_sink_with_type(&self, actor_type: Option<&str>) -> Option<Arc<MetricsSink>> {
    let sink = self
      .actor_system()
      .metrics_foreach(|runtime| Arc::new(runtime.sink_for_actor(actor_type)))?;
    match self.metrics_sink.set(sink.clone()) {
      Ok(()) => Some(sink),
      Err(value) => {
        drop(value);
        self.metrics_sink.get().cloned()
      }
    }
  }

  fn metrics_sink_or_init(&self) -> Option<Arc<MetricsSink>> {
    if let Some(existing) = self.metrics_sink.get() {
      return Some(existing.clone());
    }
    let actor_type = self.actor_type.get().map(|s| s.as_ref());
    self.init_metrics_sink_with_type(actor_type)
  }

  pub fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self.metrics_sink_or_init()
  }

  pub fn actor_type_str(&self) -> Option<&str> {
    self.actor_type.get().map(|s| s.as_ref())
  }

  pub fn actor_type_arc(&self) -> Option<Arc<str>> {
    self.actor_type.get().cloned()
  }

  fn supervisor_handle_with_snapshot(&self) -> SupervisorHandle {
    let supervisor_clone = self.clone();
    let supervisor_arc: Arc<dyn Supervisor> = Arc::new(supervisor_clone);
    let handle = SupervisorHandle::new_arc_with_metrics(supervisor_arc.clone(), self.metrics_runtime.clone());
    handle.inject_snapshot(supervisor_arc);
    handle
  }

  pub fn core_supervisor_adapter(&self) -> StdSupervisorAdapter {
    self.supervisor_handle_with_snapshot().core_adapter()
  }

  pub fn core_supervisor_strategy(&self) -> CoreSupervisorStrategyHandle {
    self.core_props().supervisor_strategy_handle()
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

    let context_handle = self.base_context_handle();
    let extras = ActorContextExtras::new(context_handle.clone());

    let extras_cell = self.extras_cell();
    let mut guard = extras_cell.write().await;
    if let Some(existing) = guard.as_ref() {
      return existing.clone();
    }
    *guard = Some(extras.clone());
    extras
  }

  async fn prepare_context_handle(&mut self) -> ContextHandle {
    if let Some(decorator) = self.props_ref().get_context_decorator_chain() {
      let extras = self.ensure_extras().await;
      let base_handle = if let Some(handle) = extras.get_context().await {
        handle
      } else {
        let handle = self.base_context_handle();
        extras.set_context(handle.clone()).await;
        handle
      };
      let decorated = decorator.run(base_handle).await;
      extras.set_context(decorated.clone()).await;
      decorated
    } else {
      self.base_context_handle()
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
      self.base_context_handle()
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
      let borrow = self.borrow();
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
    let ch = self.base_context_handle();
    let actor = self.props_ref().get_producer().run(ch).await;
    let actor_type = actor.type_name_arc();
    if self.actor_type.get().is_none() {
      let _ = self.actor_type.set(actor_type.clone());
    }
    self.set_actor(actor);

    let sink = if let Some(existing) = self.metrics_sink.get() {
      Some(existing.clone())
    } else {
      self.init_metrics_sink_with_type(Some(actor_type.as_ref()))
    };
    if let Some(sink) = sink {
      sink.increment_actor_spawn();
    }
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
          SenderContextHandle::from_context(refreshed)
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
    self
      .load_message_arc()
      .map(|handle| handle.as_ref().clone())
      .expect("message not found")
  }

  async fn set_message_or_envelope(&mut self, message_handle: MessageHandle) {
    let swap = self.message_swap();
    if let Some(mut existing) = swap.load_full() {
      if Arc::strong_count(&existing) == 1 {
        let slot = Arc::make_mut(&mut existing);
        *slot = message_handle;
        swap.store(Some(existing));
        return;
      }
    }
    swap.store(Some(Arc::new(message_handle)));
  }

  async fn reset_message_or_envelope(&mut self) {
    self.message_swap().store(None);
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
    let self_pid = self.get_self_opt().await;
    let message = SystemMessage::terminate(self_pid.as_ref().map(ExtendedPid::to_core), TerminateReason::Stopped);
    let other_stopped = MessageHandle::new(message);
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
    if let Some(extras) = self.get_extras().await {
      let children = extras.get_children().await;
      if children.is_empty().await {
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

    if let Some(sink) = self.metrics_sink() {
      sink.increment_actor_restarted();
    }
    Ok(())
  }

  async fn handle_watch(&mut self, watch: &WatchMessage) {
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::from_core(watch.watcher().clone());
    let watchers = extras.get_watchers().await;
    watchers.add(pid.inner_pid).await;
  }

  async fn handle_unwatch(&mut self, unwatch: &UnwatchMessage) {
    let extras = self.ensure_extras().await;
    let pid = ExtendedPid::from_core(unwatch.watcher().clone());
    let watchers = extras.get_watchers().await;
    watchers.remove(&pid.inner_pid).await;
  }

  async fn handle_child_failure(&mut self, f: &Failure) {
    let mut actor = self.get_actor().await.unwrap();
    let actor_system = self.actor_system();
    let supervisor_handle = self.supervisor_handle_with_snapshot();
    let core_context = StdSupervisorContext::new(actor_system.clone());
    let core_supervisor = supervisor_handle.core_adapter();
    let child_core = f.who.to_core();
    let mut tracker = f.restart_stats.to_core_tracker().await;
    let reason_core = f.reason.as_core().clone();
    let message_handle = f.message_handle.clone();

    if let Some(strategy) = actor.get_supervisor_strategy().await {
      strategy
        .core_strategy()
        .handle_child_failure(
          &core_context,
          &core_supervisor,
          child_core.clone(),
          &mut tracker,
          reason_core.clone(),
          message_handle.clone(),
        )
        .await;
    } else {
      self
        .props_ref()
        .get_supervisor_strategy()
        .core_strategy()
        .handle_child_failure(
          &core_context,
          &core_supervisor,
          child_core.clone(),
          &mut tracker,
          reason_core.clone(),
          message_handle.clone(),
        )
        .await;
    }

    f.restart_stats.overwrite_with(tracker).await;
  }

  async fn handle_terminated(&mut self, terminated_message: &TerminatedMessage) -> Result<(), ActorError> {
    if let Some(mut extras) = self.get_extras().await {
      if let Some(core_pid) = terminated_message.who.as_ref() {
        let pid = ExtendedPid::from_core(core_pid.clone());
        extras.remove_child(&pid).await;
      }
    }

    let msg = MessageHandle::new(AutoReceiveMessage::Terminated(terminated_message.clone()));
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
    let supervisor_handle = self.supervisor_handle_with_snapshot();
    let core_context = StdSupervisorContext::new(actor_system.clone());
    let core_supervisor = supervisor_handle.core_adapter();
    let mut tracker = failure.restart_stats.to_core_tracker().await;
    let reason_core = failure.reason.as_core().clone();
    let message_handle = failure.message_handle.clone();

    DEFAULT_SUPERVISION_STRATEGY
      .core_strategy()
      .handle_child_failure(
        &core_context,
        &core_supervisor,
        self.get_self_opt().await.unwrap().to_core(),
        &mut tracker,
        reason_core,
        message_handle,
      )
      .await;

    failure.restart_stats.overwrite_with(tracker).await;
  }
}

#[cfg(test)]
impl ActorContext {
  pub(crate) async fn inject_message_for_test(&self, message_handle: MessageHandle) {
    let swap = self.message_swap();
    if let Some(mut existing) = swap.load_full() {
      if Arc::strong_count(&existing) == 1 {
        let slot = Arc::make_mut(&mut existing);
        *slot = message_handle;
        swap.store(Some(existing));
        return;
      }
    }
    swap.store(Some(Arc::new(message_handle)));
  }

  pub(crate) async fn clear_message_for_test(&self) {
    self.message_swap().store(None);
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
    self.self_pid_swap().load_full().map(|inner| (*inner).clone())
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    self.self_pid_swap().store(Some(Arc::new(pid)));
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.shared.swap().load_full().map(|inner| (*inner).clone())
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
    if let Some(sender) = self.try_sender() {
      let mut cloned = self.clone();
      tracing::info!("ActorContext::respond: pid = {:?}", sender);
      cloned.send(sender, mh).await
    } else {
      tracing::info!("ActorContext::respond: sender is none");
      self
        .actor_system()
        .get_dead_letter()
        .await
        .send_user_message(None, mh)
        .await;
    }
  }

  async fn stash(&mut self) {
    let extra = self.ensure_extras().await;
    let stash = extra.get_stash().await;
    stash
      .push(self.get_message_handle_opt().await.expect("message not found"))
      .await;
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    if let Some(extras) = self.get_extras().await {
      let stash = extras.get_stash().await;
      while !stash.is_empty().await {
        let msg = stash.pop().await.unwrap();
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
    self.watch_core(&pid.to_core()).await;
  }

  async fn watch_core(&mut self, pid: &CorePid) {
    let watcher = self.get_self_opt().await.unwrap();
    let message = SystemMessage::watch(watcher.to_core());
    ExtendedPid::from(pid.clone())
      .send_system_message(self.actor_system(), MessageHandle::new(message))
      .await;
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    self.unwatch_core(&pid.to_core()).await;
  }

  async fn unwatch_core(&mut self, pid: &CorePid) {
    let watcher = self.get_self_opt().await.unwrap();
    let message = SystemMessage::unwatch(watcher.to_core());
    ExtendedPid::from(pid.clone())
      .send_system_message(self.actor_system(), MessageHandle::new(message))
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

    let extra = self.ensure_extras().await;
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
    self.forward_core(&pid.to_core()).await;
  }

  async fn forward_core(&self, pid: &CorePid) {
    if let Some(message_arc) = self.load_message_arc() {
      let message = message_arc.as_ref();
      if let Some(sm) = message.to_typed::<SystemMessage>() {
        panic!("SystemMessage cannot be forwarded: {:?}", sm);
      } else {
        ExtendedPid::from(pid.clone())
          .send_user_message(self.actor_system(), message.clone())
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
    self.try_message_envelope()
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.try_message_handle()
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.try_message_header()
  }
}

#[async_trait]
impl SenderPart for ActorContext {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    self.try_sender()
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
        let children = extras.get_children().await;
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
    if let Some(sink) = self.metrics_sink() {
      sink.increment_actor_stopped();
    }
    let actor_system = self.actor_system();
    pid.ref_process(actor_system).await.stop(pid).await;
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let actor_system = self.actor_system();
    let future_process = ActorFutureProcess::new(actor_system.clone(), timeout).await;
    let future_pid = future_process.get_pid().await.clone();
    let message = SystemMessage::watch(future_pid.to_core());
    pid.send_system_message(actor_system, MessageHandle::new(message)).await;
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

    let future_pid = future_process.get_pid().await.clone();
    let message = SystemMessage::watch(future_pid.to_core());
    pid.send_system_message(actor_system, MessageHandle::new(message)).await;
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
#[async_trait]
impl CoreSenderPart for ActorContext {
  async fn get_sender_core(&self) -> Option<CorePid> {
    self.get_sender().await.map(|pid| pid.to_core())
  }

  async fn send_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
    self.send(ExtendedPid::from(pid), message_handle).await
  }

  async fn request_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
    self.request(ExtendedPid::from(pid), message_handle).await
  }

  async fn request_with_custom_sender_core(&mut self, pid: CorePid, message_handle: MessageHandle, sender: CorePid) {
    self
      .request_with_custom_sender(ExtendedPid::from(pid), message_handle, ExtendedPid::from(sender))
      .await
  }

  async fn request_future_core(&self, pid: CorePid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    self
      .request_future(ExtendedPid::from(pid), message_handle, timeout)
      .await
  }
}
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

    let metrics_sink = self.metrics_sink();
    let message_type = metrics_sink.as_ref().map(|_| message_handle.get_type_name());
    let result = if let Some(sink) = metrics_sink.as_ref() {
      let start = Instant::now();
      let result = self.process_message(message_handle).await;
      let duration = start.elapsed();
      sink.record_message_receive_duration_with_type(duration.as_secs_f64(), message_type.as_deref());
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

  async fn record_mailbox_queue_latency(&mut self, queue: MailboxQueueKind, latency: Duration) {
    if let Some(sink) = self.metrics_sink() {
      sink.record_mailbox_queue_dwell_duration(latency.as_secs_f64(), queue.as_str());
    }
  }

  async fn record_mailbox_queue_latency_snapshot(&mut self, metrics: MailboxQueueLatencyMetrics) {
    if let Some(sink_arc) = self.metrics_sink() {
      let sink = sink_arc.as_ref();
      let mut state = self.mailbox_metrics_state.write().await;
      for queue in [MailboxQueueKind::User, MailboxQueueKind::System] {
        if metrics.total_samples(queue) == 0 {
          continue;
        }

        for (label, percentile) in [("p50", 50.0), ("p95", 95.0), ("p99", 99.0)] {
          if let Some(duration) = metrics.percentile(queue, percentile) {
            sink.record_mailbox_queue_dwell_percentile(label, duration.as_secs_f64(), queue.as_str());
          }
        }

        let bucket_counts = metrics.bucket_counts(queue);
        let bucket_state = match queue {
          MailboxQueueKind::User => &mut state.user_bucket_counts,
          MailboxQueueKind::System => &mut state.system_bucket_counts,
        };
        if bucket_state.len() < bucket_counts.len() {
          bucket_state.resize(bucket_counts.len(), 0);
        }
        for (idx, &count) in bucket_counts.iter().enumerate() {
          if bucket_state[idx] != count {
            let bucket_label = format!("bucket_{idx}");
            sink.record_mailbox_queue_dwell_bucket_total(&bucket_label, queue.as_str(), count);
            bucket_state[idx] = count;
          }
        }
      }
    }
  }

  async fn record_mailbox_suspension_metrics(&mut self, metrics: MailboxSuspensionMetrics, suspended: bool) {
    if let Some(sink_arc) = self.metrics_sink() {
      let sink = sink_arc.as_ref();
      let mut state = self.mailbox_metrics_state.write().await;
      let delta_events = metrics.resume_events.saturating_sub(state.last_resume_events);
      if delta_events > 0 {
        sink.increment_mailbox_suspension_resume(delta_events);
      }
      let total_secs = metrics.total_duration.as_secs_f64();
      let delta_secs = (total_secs - state.last_total_suspension_secs).max(0.0);
      if delta_secs > 0.0 {
        sink.record_mailbox_suspension_duration(delta_secs);
      }
      if state.last_suspended_state != suspended {
        sink.record_mailbox_suspension_state(suspended);
      }
      state.last_resume_events = metrics.resume_events;
      state.last_total_suspension_secs = total_secs;
      state.last_suspended_state = suspended;
    }
  }

  async fn record_mailbox_queue_length(&mut self, queue: MailboxQueueKind, length: u64) {
    if let Some(sink) = self.metrics_sink() {
      sink.record_mailbox_queue_length(length, queue.as_str());
    }
  }

  async fn escalate_failure(&mut self, reason: ErrorReason, message_handle: MessageHandle) {
    tracing::info!("[ACTOR] Recovering: reason = {:?}", reason.backtrace(),);

    if let Some(sink) = self.metrics_sink() {
      sink.increment_actor_failure();
    }

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

  async fn get_children(&self) -> Vec<CorePid> {
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
      .map(|pid| pid.to_core())
      .collect()
  }

  async fn escalate_failure(&self, reason: ErrorReason, message_handle: MessageHandle) {
    let self_pid = self.get_self_opt().await.expect("Failed to retrieve self_pid");
    let actor_system = self.actor_system();
    if actor_system.get_config().developer_supervision_logging {
      tracing::error!(
        "[Supervision] Actor: {}, failed with message: {}, exception: {}",
        self_pid,
        message_handle,
        reason
      );
    }

    if let Some(sink) = self.metrics_sink() {
      sink.increment_actor_failure();
    }

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

  async fn restart_children(&self, pids: &[CorePid]) {
    let actor_system = self.actor_system();
    for pid in ExtendedPid::from_core_slice(pids) {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(SystemMessage::Restart))
        .await;
    }
  }

  async fn stop_children(&self, pids: &[CorePid]) {
    let actor_system = self.actor_system();
    for pid in ExtendedPid::from_core_slice(pids) {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(SystemMessage::Stop))
        .await;
    }
  }

  async fn resume_children(&self, pids: &[CorePid]) {
    let actor_system = self.actor_system();
    for pid in ExtendedPid::from_core_slice(pids) {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(MailboxMessage::ResumeMailbox))
        .await;
    }
  }
}
