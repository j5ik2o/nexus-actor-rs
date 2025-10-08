use nexus_actor_core_rs::{ActorSystem, ActorSystemRunner, RuntimeComponents, ShutdownToken};

use crate::{FailureEventHub, TokioMailboxRuntime, TokioSpawner, TokioTimer};
use nexus_actor_core_rs::{ActorRef, MessageEnvelope, PriorityEnvelope, Props};
use nexus_utils_std_rs::QueueError;

/// ランタイム差し替え抽象のスパイク用スケルトン。
pub struct TokioActorRuntime<U>
where
  U: nexus_utils_std_rs::Element, {
  system: ActorSystem<U, TokioMailboxRuntime>,
  shutdown: ShutdownToken,
  spawner: TokioSpawner,
  timer: TokioTimer,
  events: FailureEventHub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
  Busy,
  Idle,
}

impl<U> TokioActorRuntime<U>
where
  U: nexus_utils_std_rs::Element,
{
  pub fn new() -> Self {
    let components = RuntimeComponents::new(
      TokioMailboxRuntime::default(),
      TokioSpawner,
      TokioTimer,
      FailureEventHub::new(),
    );
    let (system, handles) = ActorSystem::from_runtime_components(components);
    let shutdown = system.shutdown_token();

    Self {
      system,
      shutdown,
      spawner: handles.spawner,
      timer: handles.timer,
      events: handles.event_stream,
    }
  }

  /// 現状は ActorSystem への直接アクセスを許可（今後 Facade で包む想定）。
  pub fn system(&mut self) -> &mut ActorSystem<U, TokioMailboxRuntime> {
    &mut self.system
  }

  pub fn spawner(&self) -> &TokioSpawner {
    &self.spawner
  }

  pub fn timer(&self) -> &TokioTimer {
    &self.timer
  }

  pub fn event_stream(&self) -> &FailureEventHub {
    &self.events
  }

  /// Tokio 実装では scheduler を内部タスクで駆動するため、このメソッドは常に Idle を返す。
  pub fn pump(&mut self) -> DriverState {
    DriverState::Idle
  }

  pub fn spawn_actor(
    &mut self,
    props: Props<U, TokioMailboxRuntime>,
  ) -> Result<ActorRef<U, TokioMailboxRuntime>, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let mut root = self.system.root_context();
    root.spawn(props)
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.system.dispatch_next().await
  }

  pub fn run_until_idle(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.system.run_until_idle()
  }

  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  pub fn into_runner(self) -> ActorSystemRunner<U, TokioMailboxRuntime> {
    self.system.into_runner()
  }
}

impl<U> Default for TokioActorRuntime<U>
where
  U: nexus_utils_std_rs::Element,
{
  fn default() -> Self {
    Self::new()
  }
}
