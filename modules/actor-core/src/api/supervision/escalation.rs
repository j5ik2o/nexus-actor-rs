use alloc::sync::Arc;
use core::marker::PhantomData;

use nexus_utils_core_rs::Element;

use super::failure::FailureEvent;
use crate::{FailureInfo, MailboxFactory, PriorityEnvelope};

/// 失敗イベントを外部へ通知するためのハンドラ。
pub type FailureEventHandler = Arc<dyn Fn(&FailureInfo) + Send + Sync>;

/// 失敗イベントをストリームとして受信するためのリスナー。
pub type FailureEventListener = Arc<dyn Fn(FailureEvent) + Send + Sync>;

/// FailureInfo をどのように上位へ伝達するかを制御するためのシンク。
pub trait EscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// `already_handled` が `true` の場合は、既にローカルで FailureInfo に対する処理が完了していることを示す。
  /// 追加通知のみ行いたい場合は `Ok(())` を返す。
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo>;
}

/// ルートガーディアン用の EscalationSink 実装。
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
  pub fn new() -> Self {
    Self {
      event_handler: None,
      event_listener: None,
      _marker: PhantomData,
    }
  }

  pub fn set_event_handler(&mut self, handler: Option<FailureEventHandler>) {
    self.event_handler = handler;
  }

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
