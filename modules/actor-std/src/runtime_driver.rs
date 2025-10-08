use core::convert::Infallible;
use core::marker::PhantomData;

use nexus_actor_core_rs::{ActorSystemRunner, ShutdownToken};
use tokio::signal;
use tokio::task::JoinHandle;

use crate::TokioMailboxFactory;
use nexus_actor_core_rs::{PriorityEnvelope, RuntimeMessage};
use nexus_utils_std_rs::QueueError;

pub struct TokioSystemHandle<U>
where
  U: nexus_utils_std_rs::Element, {
  join: tokio::task::JoinHandle<Result<Infallible, QueueError<PriorityEnvelope<RuntimeMessage>>>>,
  shutdown: ShutdownToken,
  _marker: PhantomData<U>,
}

impl<U> TokioSystemHandle<U>
where
  U: nexus_utils_std_rs::Element,
{
  pub fn start_local(runner: ActorSystemRunner<U, TokioMailboxFactory>) -> Self
  where
    U: nexus_utils_std_rs::Element + 'static, {
    let shutdown = runner.shutdown_token();
    let join = tokio::task::spawn_local(async move { runner.run_forever().await });
    Self {
      join,
      shutdown,
      _marker: PhantomData,
    }
  }

  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  pub fn trigger_shutdown(&self) {
    self.shutdown.trigger();
  }

  pub async fn await_terminated(
    self,
  ) -> Result<Result<Infallible, QueueError<PriorityEnvelope<RuntimeMessage>>>, tokio::task::JoinError> {
    self.join.await
  }

  pub fn abort(self) {
    self.join.abort();
  }

  pub fn spawn_ctrl_c_listener(&self) -> JoinHandle<()>
  where
    U: nexus_utils_std_rs::Element + 'static, {
    let token = self.shutdown.clone();
    tokio::spawn(async move {
      if signal::ctrl_c().await.is_ok() {
        token.trigger();
      }
    })
  }
}
