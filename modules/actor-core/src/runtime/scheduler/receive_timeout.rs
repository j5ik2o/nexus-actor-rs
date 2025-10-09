use alloc::boxed::Box;
use alloc::sync::Arc;
use core::time::Duration;

use nexus_utils_core_rs::Element;

use crate::runtime::context::MapSystemFn;
use crate::{MailboxFactory, PriorityEnvelope, QueueMailboxProducer};

/// アクターの `ReceiveTimeout` を管理するスケジューラ抽象。
///
/// `actor-core` がランタイム依存のタイマーを直接扱わずに済むよう、
/// タイムアウトのセット / リセット / 停止を統一的なインターフェースに切り出している。
/// ユーザーメッセージ処理後に `notify_activity` を呼ぶことで、
/// ランタイム側は任意の実装（tokio / 組み込みソフトウェアタイマー等）で再アームすればよい。
pub trait ReceiveTimeoutScheduler: Send {
  /// 指定した継続時間でタイマーをセット／再アームする。
  fn set(&mut self, duration: Duration);

  /// タイマーを停止する。
  fn cancel(&mut self);

  /// ユーザーメッセージなど、タイムアウトをリセットすべきアクティビティを通知する。
  fn notify_activity(&mut self);
}

/// スケジューラを生成するためのファクトリ。
///
/// アクター生成時に優先度付きメールボックスと SystemMessage 変換関数を受け取り、
/// ランタイム固有の `ReceiveTimeoutScheduler` を組み立てる役目を担う。
/// `ActorSystem::set_receive_timeout_scheduler_factory` から登録することで、
/// すべてのアクターが同じ方針でタイムアウトを扱える。
pub trait ReceiveTimeoutSchedulerFactory<M, R>: Send + Sync
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// 優先度付きメールボックスと SystemMessage 変換関数を受け取り、アクター専用のスケジューラを作成する。
  fn create(
    &self,
    sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    map_system: Arc<MapSystemFn<M>>,
  ) -> Box<dyn ReceiveTimeoutScheduler>;
}
