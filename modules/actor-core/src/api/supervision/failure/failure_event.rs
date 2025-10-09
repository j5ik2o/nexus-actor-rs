use super::FailureInfo;

/// 障害イベント。アクターシステム内で発生した障害を表す。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureEvent {
  /// ルートアクターまでエスカレートされた障害。
  /// これ以上エスカレートできない場合に使用される。
  RootEscalated(FailureInfo),
}
