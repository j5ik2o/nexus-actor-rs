//! リモートメッセージング機能のコア実装。
//!
//! このクレートは将来のリモートメッセージングロジックのためのプレースホルダーです。
//! 現在の目標はFailureEventStreamの統合ポイントを提供することです。

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_clone)]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enum)]
#![deny(clippy::no_effect)]
#![deny(clippy::drop_copy)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::wrong_pub_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream, FailureInfo, FailureMetadata};

/// リモートアクターの障害イベントを通知するためのハンドラ。
///
/// `FailureEventStream`を使用して障害情報を配信し、カスタムハンドラによる追加処理を可能にします。
#[cfg(feature = "std")]
pub struct RemoteFailureNotifier<E>
where
  E: FailureEventStream, {
  hub: E,
  handler: Option<FailureEventListener>,
}

#[cfg(feature = "std")]
impl<E> RemoteFailureNotifier<E>
where
  E: FailureEventStream,
{
  /// 新しい`RemoteFailureNotifier`を作成します。
  ///
  /// # Arguments
  ///
  /// * `hub` - 障害イベントを配信するためのストリーム
  pub fn new(hub: E) -> Self {
    Self { hub, handler: None }
  }

  /// イベントストリームのリスナーを取得します。
  ///
  /// # Returns
  ///
  /// `FailureEventListener`のインスタンス
  pub fn listener(&self) -> FailureEventListener {
    self.hub.listener()
  }

  /// イベントハブへの参照を取得します。
  ///
  /// # Returns
  ///
  /// `FailureEventStream`への参照
  pub fn hub(&self) -> &E {
    &self.hub
  }

  /// 設定されているカスタムハンドラを取得します。
  ///
  /// # Returns
  ///
  /// ハンドラが設定されている場合は`Some(&FailureEventListener)`、そうでない場合は`None`
  pub fn handler(&self) -> Option<&FailureEventListener> {
    self.handler.as_ref()
  }

  /// カスタムハンドラを設定します。
  ///
  /// # Arguments
  ///
  /// * `handler` - 障害イベントを処理するハンドラ
  pub fn set_handler(&mut self, handler: FailureEventListener) {
    self.handler = Some(handler);
  }

  /// カスタムハンドラが設定されている場合、障害情報をディスパッチします。
  ///
  /// # Arguments
  ///
  /// * `info` - 障害情報
  pub fn dispatch(&self, info: FailureInfo) {
    if let Some(handler) = self.handler.as_ref() {
      handler(FailureEvent::RootEscalated(info.clone()));
    }
  }

  /// 障害情報をイベントハブとカスタムハンドラの両方に送信します。
  ///
  /// # Arguments
  ///
  /// * `info` - 障害情報
  pub fn emit(&self, info: FailureInfo) {
    self.hub.listener()(FailureEvent::RootEscalated(info.clone()));
    self.dispatch(info);
  }
}

/// エンドポイント情報を持つプレースホルダーのメタデータを作成します。
///
/// # Arguments
///
/// * `endpoint` - エンドポイント名
///
/// # Returns
///
/// エンドポイント情報が設定された`FailureMetadata`
pub fn placeholder_metadata(endpoint: &str) -> FailureMetadata {
  FailureMetadata::new().with_endpoint(endpoint.to_owned())
}

#[cfg(test)]
mod tests;
