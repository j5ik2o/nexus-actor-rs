#![cfg(feature = "alloc")]

use alloc::string::String;
use core::fmt::{Display, Formatter};

/// no_std + alloc 環境で扱える軽量な PID データ表現。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CorePid {
  address: String,
  id: String,
  request_id: u32,
}

/// `CorePid` を参照する型が共通で実装すべきインターフェース。
///
/// `ExtendedPid` など std 層で定義される PID ラッパーから、no_std 側の
/// `CorePid` を再利用するための抽象境界として利用する。
pub trait CorePidRef {
  /// `CorePid` への参照を返す。
  fn as_core_pid(&self) -> &CorePid;

  /// 参照先の `CorePid` を複製して返す。デフォルト実装では `Clone` を活用する。
  fn to_core_pid(&self) -> CorePid {
    self.as_core_pid().clone()
  }
}

impl CorePidRef for CorePid {
  fn as_core_pid(&self) -> &CorePid {
    self
  }
}

impl CorePid {
  #[must_use]
  pub fn new(address: impl Into<String>, id: impl Into<String>) -> Self {
    Self {
      address: address.into(),
      id: id.into(),
      request_id: 0,
    }
  }

  #[must_use]
  pub const fn with_request_id(mut self, request_id: u32) -> Self {
    self.request_id = request_id;
    self
  }

  #[must_use]
  pub fn address(&self) -> &str {
    &self.address
  }

  #[must_use]
  pub fn id(&self) -> &str {
    &self.id
  }

  #[must_use]
  pub const fn request_id(&self) -> u32 {
    self.request_id
  }

  #[must_use]
  pub fn into_parts(self) -> (String, String, u32) {
    (self.address, self.id, self.request_id)
  }
}

impl Display for CorePid {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "{}-{}-{}", self.address, self.id, self.request_id)
  }
}
