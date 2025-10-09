use core::future::Future;
use core::time::Duration;

use crate::FailureEventStream;

/// `ActorSystem` 初期化時に必要な依存集合。
///
/// アクターシステムの構築に必要なすべてのコンポーネントをまとめて保持します。
pub struct ActorSystemParts<MF, S, T, E> {
  /// メールボックスファクトリ
  pub mailbox_factory: MF,
  /// タスクスポーナー
  pub spawner: S,
  /// タイマー
  pub timer: T,
  /// イベントストリーム
  pub event_stream: E,
}

impl<MF, S, T, E> ActorSystemParts<MF, S, T, E> {
  /// 新しい `ActorSystemParts` を作成します。
  ///
  /// # 引数
  ///
  /// * `mailbox_factory` - メールボックスファクトリ
  /// * `spawner` - タスクスポーナー
  /// * `timer` - タイマー
  /// * `event_stream` - イベントストリーム
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
///
/// メールボックスファクトリを除いた、ランタイム操作に必要なハンドル群。
pub struct ActorSystemHandles<S, T, E> {
  /// タスクスポーナー
  pub spawner: S,
  /// タイマー
  pub timer: T,
  /// イベントストリーム
  pub event_stream: E,
}

impl<MF, S, T, E> ActorSystemParts<MF, S, T, E>
where
  E: FailureEventStream,
{
  /// `ActorSystemParts` をメールボックスファクトリとハンドル群に分割します。
  ///
  /// # 戻り値
  ///
  /// メールボックスファクトリと `ActorSystemHandles` のタプル
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
///
/// 環境に依存しない方法で非同期タスクを生成するための抽象層を提供します。
pub trait Spawn {
  /// 新しい非同期タスクを生成します。
  ///
  /// # 引数
  ///
  /// * `fut` - 実行する非同期タスク
  fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static);
}

/// 汎用的なタイマー抽象。
///
/// 環境に依存しない方法で遅延実行を実現するための抽象層を提供します。
pub trait Timer {
  /// スリープ操作の Future 型
  type SleepFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  /// 指定された時間だけスリープする Future を返します。
  ///
  /// # 引数
  ///
  /// * `duration` - スリープする時間
  fn sleep(&self, duration: Duration) -> Self::SleepFuture<'_>;
}
