use core::future::Future;
use core::time::Duration;

use crate::FailureEventStream;

/// `ActorSystem` 初期化時に必要な依存集合。
pub struct ActorSystemParts<MF, S, T, E> {
  pub mailbox_factory: MF,
  pub spawner: S,
  pub timer: T,
  pub event_stream: E,
}

impl<MF, S, T, E> ActorSystemParts<MF, S, T, E> {
  pub fn new(mailbox_factory: MF, spawner: S, timer: T, event_stream: E) -> Self {
    Self {
      mailbox_factory,
      spawner,
      timer,
      event_stream,
    }
  }
}

/// `ActorSystem` 構築後もドライバが保持し続ける周辺コンポーネント。
pub struct ActorSystemHandles<S, T, E> {
  pub spawner: S,
  pub timer: T,
  pub event_stream: E,
}

impl<MF, S, T, E> ActorSystemParts<MF, S, T, E>
where
  E: FailureEventStream,
{
  pub fn split(self) -> (MF, ActorSystemHandles<S, T, E>) {
    let Self {
      mailbox_factory,
      spawner,
      timer,
      event_stream,
    } = self;

    let handles = ActorSystemHandles {
      spawner,
      timer,
      event_stream,
    };

    (mailbox_factory, handles)
  }
}

/// 非同期タスク実行を抽象化するインターフェース。
pub trait Spawn {
  fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static);
}

/// 汎用的なタイマー抽象。
pub trait Timer {
  type SleepFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  fn sleep(&self, duration: Duration) -> Self::SleepFuture<'_>;
}
