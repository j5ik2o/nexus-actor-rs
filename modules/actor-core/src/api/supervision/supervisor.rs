use core::fmt;

/// スーパーバイザーが返すアクション。
///
/// アクターの障害発生時に、スーパーバイザーがどのように対処するかを指示します。
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

/// スーパーバイザーの基本トレイト。
///
/// アクターの障害処理戦略を定義し、障害時の挙動を制御します。
pub trait Supervisor<M>: Send + 'static {
  /// 障害処理の前に呼び出されるフック。
  ///
  /// デフォルト実装では何もしません。
  fn before_handle(&mut self) {}

  /// 障害処理の後に呼び出されるフック。
  ///
  /// デフォルト実装では何もしません。
  fn after_handle(&mut self) {}

  /// 障害に対する処理方針を決定します。
  ///
  /// # Arguments
  ///
  /// * `_error` - 発生したエラー情報
  ///
  /// # Returns
  ///
  /// 実行すべき `SupervisorDirective`
  fn decide(&mut self, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Stop
  }
}

/// 何もしないスーパーバイザー実装。
///
/// すべての障害に対して `Resume` を返し、処理を継続します。
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopSupervisor;

impl<M> Supervisor<M> for NoopSupervisor {
  /// すべての障害に対して `Resume` を返します。
  fn decide(&mut self, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Resume
  }
}
