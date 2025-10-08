use crate::FailureInfo;
use crate::{MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::Element;

/// FailureInfo をどのように上位へ伝達するかを制御するためのシンク。
pub trait EscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// `already_handled` が true の場合はローカルで処理済みであることを示す。
  /// 追加の通知だけ行いたい場合は、戻り値を `Ok(())` にする。
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo>;
}
