use crate::FailureEventStream;

/// ランタイム実装が `ActorSystem` を構築する際に必要な依存セット。
pub struct RuntimeComponents<R, S, T, E> {
  pub runtime: R,
  pub spawner: S,
  pub timer: T,
  pub event_stream: E,
}

impl<R, S, T, E> RuntimeComponents<R, S, T, E> {
  pub fn new(runtime: R, spawner: S, timer: T, event_stream: E) -> Self {
    Self {
      runtime,
      spawner,
      timer,
      event_stream,
    }
  }
}

/// `ActorSystem` 構築後もドライバが保持し続ける周辺コンポーネント。
pub struct RuntimeComponentHandles<S, T, E> {
  pub spawner: S,
  pub timer: T,
  pub event_stream: E,
}

impl<R, S, T, E> RuntimeComponents<R, S, T, E>
where
  E: FailureEventStream,
{
  pub fn split(self) -> (R, RuntimeComponentHandles<S, T, E>) {
    let Self {
      runtime,
      spawner,
      timer,
      event_stream,
    } = self;

    let handles = RuntimeComponentHandles {
      spawner,
      timer,
      event_stream,
    };

    (runtime, handles)
  }
}
