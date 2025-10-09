use alloc::boxed::Box;
use alloc::sync::Arc;
use core::time::Duration;

use nexus_utils_core_rs::Element;

use crate::runtime::context::MapSystemFn;
use crate::{MailboxFactory, PriorityEnvelope, QueueMailboxProducer};

/// アクターの `ReceiveTimeout` を管理するスケジューラ抽象。
pub trait ReceiveTimeoutScheduler: Send {
  /// 指定した継続時間でタイマーをセット／再アームする。
  fn set(&mut self, duration: Duration);

  /// タイマーを停止する。
  fn cancel(&mut self);

  /// ユーザーメッセージなど、タイムアウトをリセットすべきアクティビティを通知する。
  fn notify_activity(&mut self);
}

/// スケジューラを生成するためのファクトリ。
pub trait ReceiveTimeoutSchedulerFactory<M, R>: Send + Sync
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  fn create(
    &self,
    sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    map_system: Arc<MapSystemFn<M>>,
  ) -> Box<dyn ReceiveTimeoutScheduler>;
}
