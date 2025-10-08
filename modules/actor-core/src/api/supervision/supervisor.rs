use core::fmt;

/// Supervisor が返すアクション。現在はテスト用途の最小限セットのみ定義する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// アクターを停止する。
  Stop,
  /// エラーを無視して処理を継続する。
  Resume,
  /// アクターを再起動する。
  Restart,
  /// 親へエスカレーションする。
  Escalate,
}

/// Supervisor の基本トレイト。
pub trait Supervisor<M>: Send + 'static {
  fn before_handle(&mut self) {}

  fn after_handle(&mut self) {}

  fn decide(&mut self, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Stop
  }
}

/// 何もしない Supervisor 実装。デフォルトで Resume を返す。
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopSupervisor;

impl<M> Supervisor<M> for NoopSupervisor {
  fn decide(&mut self, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Resume
  }
}
