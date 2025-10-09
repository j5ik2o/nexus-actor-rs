use core::fmt;

use crate::ActorId;
use crate::MailboxFactory;
use crate::SupervisorDirective;
use nexus_utils_core_rs::Element;

/// Supervisor 戦略。protoactor-go の Strategy に相当する。
///
/// アクターの障害発生時に適用される戦略を定義するトレイトです。
/// 親アクター（ガーディアン）が子アクターの障害をどのように処理するかを決定します。
///
/// # 型パラメータ
/// - `M`: メールボックスで処理されるメッセージ型
/// - `R`: メールボックスを生成するファクトリー型
pub trait GuardianStrategy<M, R>: Send + 'static
where
  M: Element,
  R: MailboxFactory, {
  /// アクターに障害が発生した際の処理方針を決定します。
  ///
  /// # Arguments
  /// - `actor`: 障害が発生したアクターのID
  /// - `error`: 発生したエラーの詳細情報
  ///
  /// # Returns
  /// スーパーバイザーディレクティブ（Restart、Stop、Resume、Escalateなど）
  fn decide(&mut self, actor: ActorId, error: &dyn fmt::Debug) -> SupervisorDirective;

  /// アクター起動前に呼び出されるフック。
  ///
  /// デフォルト実装は何もしません。必要に応じてオーバーライドしてください。
  ///
  /// # Arguments
  /// - `_actor`: 起動するアクターのID
  fn before_start(&mut self, _actor: ActorId) {}

  /// アクター再起動後に呼び出されるフック。
  ///
  /// デフォルト実装は何もしません。必要に応じてオーバーライドしてください。
  ///
  /// # Arguments
  /// - `_actor`: 再起動したアクターのID
  fn after_restart(&mut self, _actor: ActorId) {}
}

/// 最も単純な戦略: 常に Restart を指示する。
///
/// エラーの種類に関わらず、常にアクターの再起動を指示するスーパーバイザー戦略です。
/// 一時的な障害からの自動復旧を期待する場合に適しています。
///
/// # 使用例
/// ```ignore
/// let strategy = AlwaysRestart;
/// // この戦略を使用するガーディアンは、子アクターが障害を起こしても
/// // 常に再起動を試みます
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct AlwaysRestart;

impl<M, R> GuardianStrategy<M, R> for AlwaysRestart
where
  M: Element,
  R: MailboxFactory,
{
  fn decide(&mut self, _actor: ActorId, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Restart
  }
}
