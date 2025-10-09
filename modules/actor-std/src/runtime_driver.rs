use core::convert::Infallible;
use core::marker::PhantomData;

use nexus_actor_core_rs::{ActorSystemRunner, ShutdownToken};
use tokio::signal;
use tokio::task::JoinHandle;

use crate::TokioMailboxFactory;
use nexus_actor_core_rs::{PriorityEnvelope, RuntimeMessage};
use nexus_utils_std_rs::QueueError;

/// Tokio実行環境でアクターシステムを管理するハンドル
///
/// アクターシステムの起動、シャットダウン、および終了待機を制御します。
pub struct TokioSystemHandle<U>
where
  U: nexus_utils_std_rs::Element, {
  join: tokio::task::JoinHandle<Result<Infallible, QueueError<PriorityEnvelope<RuntimeMessage>>>>,
  shutdown: ShutdownToken,
  _marker: PhantomData<U>,
}

impl<U> TokioSystemHandle<U>
where
  U: nexus_utils_std_rs::Element,
{
  /// アクターシステムをローカルタスクとして起動する
  ///
  /// # Arguments
  /// * `runner` - 起動するアクターシステムのランナー
  ///
  /// # Returns
  /// アクターシステムを管理する新しい`TokioSystemHandle`
  pub fn start_local(runner: ActorSystemRunner<U, TokioMailboxFactory>) -> Self
  where
    U: nexus_utils_std_rs::Element + 'static, {
    let shutdown = runner.shutdown_token();
    let join = tokio::task::spawn_local(async move { runner.run_forever().await });
    Self {
      join,
      shutdown,
      _marker: PhantomData,
    }
  }

  /// システムのシャットダウントークンを取得する
  ///
  /// # Returns
  /// シャットダウンを制御するための`ShutdownToken`
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  /// アクターシステムのシャットダウンをトリガーする
  ///
  /// システムの優雅な終了を開始します。
  pub fn trigger_shutdown(&self) {
    self.shutdown.trigger();
  }

  /// アクターシステムの終了を待機する
  ///
  /// システムが完全に停止するまで非同期で待機します。
  ///
  /// # Returns
  /// システム実行の結果。外側の`Result`はタスクの結合エラー、
  /// 内側の`Result`はシステムの実行エラーを示します。
  pub async fn await_terminated(
    self,
  ) -> Result<Result<Infallible, QueueError<PriorityEnvelope<RuntimeMessage>>>, tokio::task::JoinError> {
    self.join.await
  }

  /// アクターシステムの実行を強制終了する
  ///
  /// 優雅なシャットダウンを行わず、即座にシステムを中断します。
  pub fn abort(self) {
    self.join.abort();
  }

  /// Ctrl+Cシグナルを監視し、受信時にシャットダウンをトリガーするタスクを起動する
  ///
  /// # Returns
  /// リスナータスクの`JoinHandle`
  pub fn spawn_ctrl_c_listener(&self) -> JoinHandle<()>
  where
    U: nexus_utils_std_rs::Element + 'static, {
    let token = self.shutdown.clone();
    tokio::spawn(async move {
      if signal::ctrl_c().await.is_ok() {
        token.trigger();
      }
    })
  }
}
