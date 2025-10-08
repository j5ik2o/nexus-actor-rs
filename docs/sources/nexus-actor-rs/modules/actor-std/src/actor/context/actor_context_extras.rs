use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::context_handle::{ContextHandle, WeakContextHandle};
use crate::actor::context::lock_timing::InstrumentedRwLock;
use crate::actor::context::receive_timeout_timer::ReceiveTimeoutTimer;
use crate::actor::context::receiver_context_handle::ReceiverContextHandle;
use crate::actor::context::sender_context_handle::SenderContextHandle;
use crate::actor::core::ExtendedPid;
use crate::actor::core::PidSet;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandles;
use crate::ctxext::extensions::ContextExtensions;
use arc_swap::ArcSwapOption;
use nexus_actor_core_rs::runtime::CoreScheduledTask;
use once_cell::sync::OnceCell;

#[derive(Debug, Clone)]
struct ActorContextExtrasMutable {
  receive_timeout_timer: Option<ReceiveTimeoutTimer>,
  receive_timeout_context: Option<Weak<RwLock<ActorContext>>>,
  restart_stats: OnceCell<RestartStatistics>,
  stash: MessageHandles,
}

impl ActorContextExtrasMutable {
  fn new() -> Self {
    Self {
      receive_timeout_timer: None,
      receive_timeout_context: None,
      restart_stats: OnceCell::new(),
      stash: MessageHandles::new(vec![]),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ActorContextExtras {
  mutable: Arc<InstrumentedRwLock<ActorContextExtrasMutable>>,
  children: PidSet,
  watchers: PidSet,
  context: Arc<ArcSwapOption<WeakContextHandle>>,
  extensions: ContextExtensions,
}

impl ActorContextExtras {
  pub fn new(context: ContextHandle) -> Self {
    let context_arc = Arc::new(ArcSwapOption::from(Some(Arc::new(context.downgrade()))));
    Self {
      mutable: Arc::new(InstrumentedRwLock::new(
        ActorContextExtrasMutable::new(),
        "ActorContextExtras::mutable",
      )),
      children: PidSet::new(),
      watchers: PidSet::new(),
      context: context_arc,
      extensions: ContextExtensions::new(),
    }
  }

  pub async fn get_receive_timeout_timer(&self) -> Option<ReceiveTimeoutTimer> {
    let guard = self.mutable.read("get_receive_timeout_timer").await;
    guard.receive_timeout_timer.clone()
  }

  pub async fn get_context(&self) -> Option<ContextHandle> {
    self.context.load_full().and_then(|weak| weak.upgrade())
  }

  pub async fn get_sender_context(&self) -> Option<SenderContextHandle> {
    self
      .context
      .load_full()
      .and_then(|weak| weak.upgrade())
      .map(SenderContextHandle::from_context)
  }

  pub async fn get_receiver_context(&self) -> Option<ReceiverContextHandle> {
    self
      .context
      .load_full()
      .and_then(|weak| weak.upgrade())
      .map(ReceiverContextHandle::new)
  }

  pub async fn set_context(&self, context: ContextHandle) {
    self.context.store(Some(Arc::new(context.downgrade())));
  }

  pub async fn get_extensions(&self) -> ContextExtensions {
    self.extensions.clone()
  }

  pub async fn get_children(&self) -> PidSet {
    self.children.clone()
  }

  pub async fn get_watchers(&self) -> PidSet {
    self.watchers.clone()
  }

  pub async fn get_stash(&self) -> MessageHandles {
    let guard = self.mutable.read("get_stash").await;
    guard.stash.clone()
  }

  pub async fn restart_stats(&mut self) -> RestartStatistics {
    let factory = self
      .context
      .load_full()
      .and_then(|weak| weak.upgrade())
      .and_then(|ctx| ctx.with_actor_context(|actor_ctx| actor_ctx.actor_system().core_runtime()));

    let guard = self.mutable.write("restart_stats").await;
    if guard.restart_stats.get().is_none() {
      let stats = runtime
        .as_ref()
        .map(RestartStatistics::with_runtime)
        .unwrap_or_else(RestartStatistics::new);
      let _ = guard.restart_stats.set(stats);
    }
    guard
      .restart_stats
      .get()
      .expect("restart stats must be initialized")
      .clone()
  }

  pub async fn init_or_reset_receive_timeout_timer(&self, duration: Duration, context: Arc<RwLock<ActorContext>>) {
    // cancel existing timer if present
    let previous = {
      let mut guard = self
        .mutable
        .write("init_or_reset_receive_timeout_timer:take_existing")
        .await;
      guard.receive_timeout_timer.take()
    };
    if let Some(timer) = previous {
      timer.cancel();
    }

    let factory = {
      let ctx_guard = context.read().await;
      ctx_guard.actor_system().core_runtime()
    };
    let scheduler = runtime.scheduler();
    let task_context = context.clone();
    let task: CoreScheduledTask = Arc::new(move || {
      let context = task_context.clone();
      Box::pin(async move {
        let mut ctx = context.write().await;
        ctx.receive_timeout_handler().await;
      })
    });

    let handle = scheduler.schedule_once(duration, task);

    let mut guard = self
      .mutable
      .write("init_or_reset_receive_timeout_timer:set_timer")
      .await;
    guard.receive_timeout_context = Some(Arc::downgrade(&context));
    guard.receive_timeout_timer = Some(ReceiveTimeoutTimer::new(handle));
  }

  pub async fn reset_receive_timeout_timer(&self, duration: Duration) {
    let context = {
      let guard = self.mutable.read("reset_receive_timeout_timer").await;
      guard.receive_timeout_context.as_ref().and_then(|weak| weak.upgrade())
    };

    if let Some(context) = context {
      self.init_or_reset_receive_timeout_timer(duration, context).await;
    }
  }

  pub async fn stop_receive_timeout_timer(&self) {
    let timer = {
      let mut guard = self.mutable.write("stop_receive_timeout_timer").await;
      guard.receive_timeout_timer.take()
    };
    if let Some(timer) = timer {
      timer.cancel();
    }
  }

  pub async fn kill_receive_timeout_timer(&self) {
    let mut guard = self.mutable.write("kill_receive_timeout_timer").await;
    if let Some(timer) = guard.receive_timeout_timer.take() {
      timer.cancel();
    }
    guard.receive_timeout_context = None;
  }

  pub async fn add_child(&mut self, pid: ExtendedPid) {
    self.children.add(pid.inner_pid).await;
  }

  pub async fn remove_child(&mut self, pid: &ExtendedPid) {
    self.children.remove(&pid.inner_pid).await;
  }
}
