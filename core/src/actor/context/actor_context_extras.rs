use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::context_handle::ContextHandle;
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

#[derive(Debug, Clone)]
struct ActorContextExtrasInner {
  children: PidSet,
  pub(crate) receive_timeout_timer: Option<ReceiveTimeoutTimer>,
  rs: Arc<RwLock<Option<RestartStatistics>>>,
  stash: MessageHandles,
  watchers: PidSet,
  context: ContextHandle,
  extensions: ContextExtensions,
}

impl ActorContextExtrasInner {
  pub async fn new(context: ContextHandle) -> Self {
    Self {
      children: PidSet::new().await,
      receive_timeout_timer: None,
      rs: Arc::new(RwLock::new(None)),
      stash: MessageHandles::new(vec![]),
      watchers: PidSet::new().await,
      context,
      extensions: ContextExtensions::new(),
    }
  }
}
#[derive(Debug, Clone)]
pub struct ActorContextExtras {
  inner: Arc<RwLock<ActorContextExtrasInner>>,
}

impl ActorContextExtras {
  pub async fn new(context: ContextHandle) -> Self {
    Self {
      inner: Arc::new(RwLock::new(ActorContextExtrasInner::new(context).await)),
    }
  }

  pub async fn get_receive_timeout_timer(&self) -> Option<ReceiveTimeoutTimer> {
    let mg = self.inner.read().await;
    mg.receive_timeout_timer.clone()
  }

  pub async fn get_context(&self) -> ContextHandle {
    let mg = self.inner.read().await;
    mg.context.clone()
  }

  pub async fn get_sender_context(&self) -> SenderContextHandle {
    let inner_mg = self.inner.read().await;
    SenderContextHandle::new(inner_mg.context.clone())
  }

  pub async fn get_receiver_context(&self) -> ReceiverContextHandle {
    let inner_mg = self.inner.read().await;
    ReceiverContextHandle::new(inner_mg.context.clone())
  }

  pub async fn get_extensions(&self) -> ContextExtensions {
    let inner_mg = self.inner.read().await;
    inner_mg.extensions.clone()
  }

  pub async fn get_children(&self) -> PidSet {
    let inner_mg = self.inner.read().await;
    inner_mg.children.clone()
  }

  pub async fn get_watchers(&self) -> PidSet {
    let inner_mg = self.inner.read().await;
    inner_mg.watchers.clone()
  }

  pub async fn get_stash(&self) -> MessageHandles {
    let inner_mg = self.inner.read().await;
    inner_mg.stash.clone()
  }

  pub async fn restart_stats(&mut self) -> RestartStatistics {
    let inner_mg = self.inner.read().await;
    let mut rs_mg = inner_mg.rs.write().await;
    if rs_mg.is_none() {
      *rs_mg = Some(RestartStatistics::new())
    }
    rs_mg.as_ref().unwrap().clone()
  }

  pub async fn init_receive_timeout_timer(&self, duration: Duration) {
    let mut inner_mg = self.inner.write().await;
    match inner_mg.receive_timeout_timer {
      Some(_) => {},
      None => {
        inner_mg.receive_timeout_timer = Some(ReceiveTimeoutTimer::new(duration));
      }
    }
  }

  pub async fn init_or_reset_receive_timeout_timer(&mut self, d: Duration, context: Arc<RwLock<ActorContext>>) {
    self.stop_receive_timeout_timer().await;

    let timer = Arc::new(RwLock::new(Box::pin(tokio::time::sleep(d))));
    {
      let mut mg = self.inner.write().await;
      mg.receive_timeout_timer = Some(ReceiveTimeoutTimer::from_underlying(timer.clone()));
    }

    let context = context.clone();
    let dispatcher = {
      let mg = context.read().await;
      mg.get_actor_system().await.get_config().await.system_dispatcher.clone()
    };

    dispatcher
      .schedule(Runnable::new(move || async move {
        let mut mg = timer.write().await;
        // FIXME: これ必要？
        mg.as_mut().await;
        let mut locked_context = context.write().await;
        locked_context.receive_timeout_handler().await;
      }))
      .await;
  }

  pub async fn reset_receive_timeout_timer(&self, duration: Duration) {
    let mut mg = self.inner.write().await;
    if let Some(t) = &mut mg.receive_timeout_timer {
      t.reset(tokio::time::Instant::now() + duration).await;
    }
  }

  pub async fn stop_receive_timeout_timer(&self) {
    let mut mg = self.inner.write().await;
    if let Some(t) = &mut mg.receive_timeout_timer {
      t.stop().await;
    }
  }

  pub async fn kill_receive_timeout_timer(&self) {
    let mut mg = self.inner.write().await;
    if mg.receive_timeout_timer.is_some() {
      mg.receive_timeout_timer = None
    }
  }

  pub async fn wait_for_timeout(&self) {
    let mg = self.inner.read().await;
    if let Some(timer) = mg.receive_timeout_timer.clone() {
      timer.wait().await;
    }

    if let Some(t) = &mg.receive_timeout_timer {
      t.wait().await
    }
  }

  pub async fn add_child(&mut self, pid: ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.children.add(pid.inner_pid).await;
  }

  pub async fn remove_child(&mut self, pid: &ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.children.remove(&pid.inner_pid).await;
  }
}
