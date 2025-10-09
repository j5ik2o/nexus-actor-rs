use alloc::sync::Arc;
use core::marker::PhantomData;

use nexus_utils_core_rs::Element;

use super::failure::FailureEvent;
use crate::{FailureInfo, MailboxFactory, PriorityEnvelope};

/// 失敗イベントを外部へ通知するためのハンドラ。
///
/// アクターの障害情報を受け取り、ログ記録や監視システムへの通知などを行います。
pub type FailureEventHandler = Arc<dyn Fn(&FailureInfo) + Send + Sync>;

/// 失敗イベントをストリームとして受信するためのリスナー。
///
/// アクターシステム全体の障害イベントを購読し、カスタム処理を実行します。
pub type FailureEventListener = Arc<dyn Fn(FailureEvent) + Send + Sync>;

/// `FailureInfo` をどのように上位へ伝達するかを制御するためのシンク。
///
/// アクターの障害情報のエスカレーション処理を定義します。
pub trait EscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// 障害情報を処理します。
  ///
  /// # Arguments
  ///
  /// * `info` - 障害情報
  /// * `already_handled` - `true` の場合、既にローカルで処理が完了していることを示す
  ///
  /// # Returns
  ///
  /// 成功した場合は `Ok(())`、処理できなかった場合は `Err(FailureInfo)`
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo>;
}

/// ルートガーディアン用の `EscalationSink` 実装。
///
/// アクターシステムのルートレベルでの障害処理を担当します。
/// これ以上エスカレーションできない障害を最終的に処理します。
pub struct RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  event_handler: Option<FailureEventHandler>,
  event_listener: Option<FailureEventListener>,
  _marker: PhantomData<(M, R)>,
}

impl<M, R> RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  /// 新しい `RootEscalationSink` を作成します。
  ///
  /// デフォルトではハンドラとリスナーは設定されていません。
  pub fn new() -> Self {
    Self {
      event_handler: None,
      event_listener: None,
      _marker: PhantomData,
    }
  }

  /// 障害イベントハンドラを設定します。
  ///
  /// # Arguments
  ///
  /// * `handler` - 障害イベントハンドラ、または `None`
  pub fn set_event_handler(&mut self, handler: Option<FailureEventHandler>) {
    self.event_handler = handler;
  }

  /// 障害イベントリスナーを設定します。
  ///
  /// # Arguments
  ///
  /// * `listener` - 障害イベントリスナー、または `None`
  pub fn set_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.event_listener = listener;
  }
}

impl<M, R> Default for RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<M, R> EscalationSink<M, R> for RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  /// ルートレベルでの障害情報を処理します。
  ///
  /// ログ出力、ハンドラ呼び出し、リスナー通知を行います。
  ///
  /// # Arguments
  ///
  /// * `info` - 障害情報
  /// * `_already_handled` - 未使用（ルートレベルでは常に処理を実行）
  ///
  /// # Returns
  ///
  /// 常に `Ok(())` を返します
  fn handle(&mut self, info: FailureInfo, _already_handled: bool) -> Result<(), FailureInfo> {
    #[cfg(feature = "std")]
    {
      use tracing::error;
      error!(
        actor = ?info.actor,
        reason = %info.reason,
        path = ?info.path.segments(),
        "actor escalation reached root guardian"
      );
    }

    if let Some(handler) = self.event_handler.as_ref() {
      handler(&info);
    }

    if let Some(listener) = self.event_listener.as_ref() {
      listener(FailureEvent::RootEscalated(info.clone()));
    }

    Ok(())
  }
}
