use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::context_handle::{ContextHandle, WeakContextHandle};
use crate::actor::context::lock_timing::InstrumentedRwLock;
use crate::actor::context::receive_timeout_timer::ReceiveTimeoutTimer;
use crate::actor::context::receiver_context_handle::ReceiverContextHandle;
use crate::actor::context::sender_context_handle::SenderContextHandle;
use crate::actor::context::InfoPart;
use crate::actor::core::ExtendedPid;
use crate::actor::core::PidSet;
use crate::actor::core::RestartStatistics;
use crate::actor::dispatch::Runnable;
use crate::actor::message::MessageHandles;
use crate::ctxext::extensions::ContextExtensions;
use arc_swap::ArcSwapOption;
use once_cell::sync::OnceCell;

#[derive(Debug, Clone)]
struct ActorContextExtrasMutable {
  receive_timeout_timer: Option<ReceiveTimeoutTimer>,
  restart_stats: OnceCell<RestartStatistics>,
  stash: MessageHandles,
}

impl ActorContextExtrasMutable {
  fn new() -> Self {
    Self {
      receive_timeout_timer: None,
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
    let guard = self.mutable.write("restart_stats").await;
    guard.restart_stats.get_or_init(RestartStatistics::new).clone()
  }

  pub async fn init_receive_timeout_timer(&self, duration: Duration) {
    let mut guard = self.mutable.write("init_receive_timeout_timer").await;
    if guard.receive_timeout_timer.is_none() {
      guard.receive_timeout_timer = Some(ReceiveTimeoutTimer::new(duration));
    }
  }

  pub async fn init_or_reset_receive_timeout_timer(&mut self, d: Duration, context: Arc<RwLock<ActorContext>>) {
    self.stop_receive_timeout_timer().await;

    let timer = Arc::new(RwLock::new(Box::pin(tokio::time::sleep(d))));
    {
      let mut guard = self
        .mutable
        .write("init_or_reset_receive_timeout_timer:set_timer")
        .await;
      guard.receive_timeout_timer = Some(ReceiveTimeoutTimer::from_underlying(timer.clone()));
    }

    let dispatcher = {
      let mg = context.read().await;
      mg.get_actor_system().await.get_config().system_dispatcher.clone()
    };

    dispatcher
      .schedule(Runnable::new(move || async move {
        let mut mg = timer.write().await;
        mg.as_mut().await;
        let mut locked_context = context.write().await;
        locked_context.receive_timeout_handler().await;
      }))
      .await;
  }

  pub async fn reset_receive_timeout_timer(&self, duration: Duration) {
    let mut timer = {
      let guard = self.mutable.read("reset_receive_timeout_timer").await;
      guard.receive_timeout_timer.clone()
    };
    if let Some(ref mut t) = timer {
      t.reset(tokio::time::Instant::now() + duration).await;
    }
  }

  pub async fn stop_receive_timeout_timer(&self) {
    let timer = {
      let guard = self.mutable.read("stop_receive_timeout_timer").await;
      guard.receive_timeout_timer.clone()
    };
    if let Some(mut t) = timer {
      t.stop().await;
    }
  }

  pub async fn kill_receive_timeout_timer(&self) {
    let mut guard = self.mutable.write("kill_receive_timeout_timer").await;
    guard.receive_timeout_timer = None;
  }

  pub async fn wait_for_timeout(&self) {
    let timer = {
      let guard = self.mutable.read("wait_for_timeout").await;
      guard.receive_timeout_timer.clone()
    };
    if let Some(t) = timer {
      t.wait().await;
    }
  }

  pub async fn add_child(&mut self, pid: ExtendedPid) {
    self.children.add(pid.inner_pid);
  }

  pub async fn remove_child(&mut self, pid: &ExtendedPid) {
    self.children.remove(&pid.inner_pid);
  }
}
