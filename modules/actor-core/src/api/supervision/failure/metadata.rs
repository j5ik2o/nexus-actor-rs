use alloc::collections::BTreeMap;
use alloc::string::String;

/// Failure に付随するメタデータ。将来 remote/cluster 層の情報を保持するために使用する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureMetadata {
  /// 障害が発生したコンポーネント名
  pub component: Option<String>,
  /// 障害が発生したエンドポイント
  pub endpoint: Option<String>,
  /// 使用されたトランスポート
  pub transport: Option<String>,
  /// 追加のタグ情報
  pub tags: BTreeMap<String, String>,
}

impl FailureMetadata {
  /// 新しい空のメタデータを作成する。
  ///
  /// # Returns
  /// 新しい`FailureMetadata`インスタンス
  pub fn new() -> Self {
    Self {
      component: None,
      endpoint: None,
      transport: None,
      tags: BTreeMap::new(),
    }
  }

  /// コンポーネント名を設定する。
  ///
  /// # Arguments
  /// * `component` - コンポーネント名
  ///
  /// # Returns
  /// コンポーネント名が設定された`FailureMetadata`インスタンス
  pub fn with_component(mut self, component: impl Into<String>) -> Self {
    self.component = Some(component.into());
    self
  }

  /// エンドポイントを設定する。
  ///
  /// # Arguments
  /// * `endpoint` - エンドポイント
  ///
  /// # Returns
  /// エンドポイントが設定された`FailureMetadata`インスタンス
  pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
    self.endpoint = Some(endpoint.into());
    self
  }

  /// トランスポートを設定する。
  ///
  /// # Arguments
  /// * `transport` - トランスポート
  ///
  /// # Returns
  /// トランスポートが設定された`FailureMetadata`インスタンス
  pub fn with_transport(mut self, transport: impl Into<String>) -> Self {
    self.transport = Some(transport.into());
    self
  }

  /// タグを追加する。
  ///
  /// # Arguments
  /// * `key` - タグのキー
  /// * `value` - タグの値
  ///
  /// # Returns
  /// タグが追加された`FailureMetadata`インスタンス
  pub fn insert_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.tags.insert(key.into(), value.into());
    self
  }
}

impl Default for FailureMetadata {
  fn default() -> Self {
    Self::new()
  }
}
