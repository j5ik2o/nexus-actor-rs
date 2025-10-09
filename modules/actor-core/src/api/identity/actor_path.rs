use alloc::vec::Vec;
use core::fmt;

use crate::ActorId;

/// アクターの階層的なパス。
///
/// アクターの位置を階層構造で表現し、ルートからのパスをセグメントの列として保持します。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorPath {
  segments: Vec<ActorId>,
}

impl ActorPath {
  /// 空の新しい `ActorPath` を作成します。
  ///
  /// セグメントを持たないルートパスを表します。
  pub fn new() -> Self {
    Self { segments: Vec::new() }
  }

  /// パスのセグメント列を取得します。
  ///
  /// # Returns
  ///
  /// `ActorId` のスライス
  pub fn segments(&self) -> &[ActorId] {
    &self.segments
  }

  /// 子アクターのIDを追加した新しいパスを作成します。
  ///
  /// # Arguments
  ///
  /// * `id` - 追加する子アクターのID
  ///
  /// # Returns
  ///
  /// 子アクターを含む新しい `ActorPath`
  pub fn push_child(&self, id: ActorId) -> Self {
    let mut segments = self.segments.clone();
    segments.push(id);
    Self { segments }
  }

  /// 親アクターのパスを取得します。
  ///
  /// # Returns
  ///
  /// 親パスが存在する場合は `Some(ActorPath)`、ルートの場合は `None`
  pub fn parent(&self) -> Option<Self> {
    if self.segments.is_empty() {
      None
    } else {
      let mut segments = self.segments.clone();
      segments.pop();
      Some(Self { segments })
    }
  }

  /// パスの最後のセグメント（アクターID）を取得します。
  ///
  /// # Returns
  ///
  /// 最後の `ActorId`、または空の場合は `None`
  pub fn last(&self) -> Option<ActorId> {
    self.segments.last().copied()
  }

  /// パスが空（ルート）かどうかを判定します。
  ///
  /// # Returns
  ///
  /// 空の場合は `true`、そうでなければ `false`
  pub fn is_empty(&self) -> bool {
    self.segments.is_empty()
  }
}

impl Default for ActorPath {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Display for ActorPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.segments.is_empty() {
      return f.write_str("/");
    }

    for segment in &self.segments {
      write!(f, "/{}", segment)?;
    }
    Ok(())
  }
}
