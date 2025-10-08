use nexus_actor_core_rs::{ActorSystem, RuntimeComponents};

use crate::{FailureEventHub, TokioMailboxRuntime, TokioSpawner, TokioTimer};
use nexus_actor_core_rs::{ActorRef, MessageEnvelope, PriorityEnvelope, Props};
use nexus_utils_std_rs::QueueError;

/// ランタイム差し替え抽象のスパイク用スケルトン。
pub struct TokioActorRuntime<U>
where
  U: nexus_utils_std_rs::Element, {
  system: ActorSystem<U, TokioMailboxRuntime>,
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

    Self {
      system,
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

  /// Tokio 実装ではポーリング不要のため常に Idle を返す想定。
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
}

impl<U> Default for TokioActorRuntime<U>
where
  U: nexus_utils_std_rs::Element,
{
  fn default() -> Self {
    Self::new()
  }
}
