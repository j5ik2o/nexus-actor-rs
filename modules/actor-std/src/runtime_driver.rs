use nexus_actor_core_rs::ActorSystem;

use crate::{FailureEventHub, TokioMailboxRuntime, TokioSpawner, TokioTimer};

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
    Self {
      system: ActorSystem::new(TokioMailboxRuntime::default()),
      spawner: TokioSpawner,
      timer: TokioTimer,
      events: FailureEventHub::new(),
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
}

impl<U> Default for TokioActorRuntime<U>
where
  U: nexus_utils_std_rs::Element,
{
  fn default() -> Self {
    Self::new()
  }
}
