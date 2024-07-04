use std::sync::Arc;

use tokio::sync::Mutex;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::pid_set::PidSet;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::receive_timeout_timer::ReceiveTimeoutTimer;
use crate::actor::context::{ContextHandle, ReceiverContextHandle, SenderContextHandle};
use crate::actor::message::message_handles::MessageHandles;
use crate::ctxext::extensions::ContextExtensions;

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
      children: PidSet::new().await,
      receive_timeout_timer: None,
      rs: Arc::new(Mutex::new(None)),
      stash: MessageHandles::new(vec![]),
      watchers: PidSet::new().await,
      context,
      extensions: ContextExtensions::new(),
    }
  }
}
#[derive(Debug, Clone)]
pub struct ActorContextExtras {
  inner: Arc<Mutex<ActorContextExtrasInner>>,
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

  pub async fn init_or_reset_receive_timeout_timer(
    &mut self,
    d: tokio::time::Duration,
    context: Arc<Mutex<ActorContext>>,
  ) {
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
