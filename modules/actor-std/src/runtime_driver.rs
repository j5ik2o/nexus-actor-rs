use core::convert::Infallible;
use core::marker::PhantomData;

use nexus_actor_core_rs::{ActorSystemRunner, ShutdownToken};
use tokio::signal;
use tokio::task::JoinHandle;

use crate::TokioMailboxFactory;
use nexus_actor_core_rs::{PriorityEnvelope, RuntimeMessage};
use nexus_utils_std_rs::QueueError;

/// Handle for managing the actor system in the Tokio execution environment
///
/// Controls the startup, shutdown, and termination waiting of the actor system.
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
  /// Starts the actor system as a local task
  ///
  /// # Arguments
  /// * `runner` - The actor system runner to start
  ///
  /// # Returns
  /// A new `TokioSystemHandle` for managing the actor system
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

  /// Returns the system's shutdown token
  ///
  /// # Returns
  /// A `ShutdownToken` for controlling shutdown
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  /// Triggers the shutdown of the actor system
  ///
  /// Initiates a graceful shutdown of the system.
  pub fn trigger_shutdown(&self) {
    self.shutdown.trigger();
  }

  /// Waits for the actor system to terminate
  ///
  /// Asynchronously waits until the system has completely stopped.
  ///
  /// # Returns
  /// The result of system execution. The outer `Result` indicates task join errors,
  /// the inner `Result` indicates system execution errors.
  pub async fn await_terminated(
    self,
  ) -> Result<Result<Infallible, QueueError<PriorityEnvelope<RuntimeMessage>>>, tokio::task::JoinError> {
    self.join.await
  }

  /// Forcibly terminates the actor system execution
  ///
  /// Aborts the system immediately without performing a graceful shutdown.
  pub fn abort(self) {
    self.join.abort();
  }

  /// Spawns a task that monitors Ctrl+C signals and triggers shutdown upon receipt
  ///
  /// # Returns
  /// A `JoinHandle` for the listener task
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
