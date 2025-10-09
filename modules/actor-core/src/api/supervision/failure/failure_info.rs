use crate::ActorId;
use crate::ActorPath;

use super::{EscalationStage, FailureMetadata};

/// 障害情報。protoactor-go の Failure メッセージを簡略化した形で保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
  /// 障害が発生したアクターのID
  pub actor: ActorId,
  /// 障害が発生したアクターのパス
  pub path: ActorPath,
  /// 障害の理由を表す文字列
  pub reason: alloc::string::String,
  /// 障害に関連するメタデータ
  pub metadata: FailureMetadata,
  /// エスカレーションの段階
  pub stage: EscalationStage,
}

impl FailureInfo {
  /// デフォルトのメタデータで新しい障害情報を作成する。
  ///
  /// # 引数
  /// * `actor` - 障害が発生したアクターのID
  /// * `path` - 障害が発生したアクターのパス
  /// * `reason` - 障害の理由を表す文字列
  ///
  /// # 戻り値
  /// 新しい`FailureInfo`インスタンス
  pub fn new(actor: ActorId, path: ActorPath, reason: alloc::string::String) -> Self {
    Self::new_with_metadata(actor, path, reason, FailureMetadata::default())
  }

  /// メタデータを指定して新しい障害情報を作成する。
  ///
  /// # 引数
  /// * `actor` - 障害が発生したアクターのID
  /// * `path` - 障害が発生したアクターのパス
  /// * `reason` - 障害の理由を表す文字列
  /// * `metadata` - 障害に関連するメタデータ
  ///
  /// # 戻り値
  /// 新しい`FailureInfo`インスタンス
  pub fn new_with_metadata(
    actor: ActorId,
    path: ActorPath,
    reason: alloc::string::String,
    metadata: FailureMetadata,
  ) -> Self {
    Self {
      actor,
      path,
      reason,
      metadata,
      stage: EscalationStage::Initial,
    }
  }

  /// メタデータを設定する。
  ///
  /// # 引数
  /// * `metadata` - 設定するメタデータ
  ///
  /// # 戻り値
  /// メタデータが設定された`FailureInfo`インスタンス
  pub fn with_metadata(mut self, metadata: FailureMetadata) -> Self {
    self.metadata = metadata;
    self
  }

  /// エスカレーション段階を設定する。
  ///
  /// # 引数
  /// * `stage` - 設定するエスカレーション段階
  ///
  /// # 戻り値
  /// エスカレーション段階が設定された`FailureInfo`インスタンス
  pub fn with_stage(mut self, stage: EscalationStage) -> Self {
    self.stage = stage;
    self
  }

  /// エラーからデフォルトのメタデータで障害情報を作成する。
  ///
  /// # 引数
  /// * `actor` - 障害が発生したアクターのID
  /// * `path` - 障害が発生したアクターのパス
  /// * `error` - エラーオブジェクト
  ///
  /// # 戻り値
  /// 新しい`FailureInfo`インスタンス
  pub fn from_error(actor: ActorId, path: ActorPath, error: &dyn core::fmt::Debug) -> Self {
    Self::from_error_with_metadata(actor, path, error, FailureMetadata::default())
  }

  /// エラーとメタデータから障害情報を作成する。
  ///
  /// # 引数
  /// * `actor` - 障害が発生したアクターのID
  /// * `path` - 障害が発生したアクターのパス
  /// * `error` - エラーオブジェクト
  /// * `metadata` - 障害に関連するメタデータ
  ///
  /// # 戻り値
  /// 新しい`FailureInfo`インスタンス
  pub fn from_error_with_metadata(
    actor: ActorId,
    path: ActorPath,
    error: &dyn core::fmt::Debug,
    metadata: FailureMetadata,
  ) -> Self {
    Self {
      actor,
      path,
      reason: alloc::format!("{:?}", error),
      metadata,
      stage: EscalationStage::Initial,
    }
  }

  /// 親アクターへエスカレートした新しい障害情報を作成する。
  ///
  /// # 戻り値
  /// 親アクターへエスカレートした`FailureInfo`インスタンス。
  /// 親が存在しない場合は`None`を返す。
  pub fn escalate_to_parent(&self) -> Option<Self> {
    let parent_path = self.path.parent()?;
    let parent_actor = parent_path.last().unwrap_or(self.actor);
    Some(Self {
      actor: parent_actor,
      path: parent_path,
      reason: self.reason.clone(),
      metadata: self.metadata.clone(),
      stage: self.stage.escalate(),
    })
  }
}
