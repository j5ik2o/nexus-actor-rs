use crate::collections::queue::traits::{QueueBase, QueueReader, QueueWriter};
use crate::collections::queue::QueueStorage;
use crate::collections::{QueueError, QueueSize};
use crate::sync::Shared;

/// リングバッファベースのキューのためのバックエンド抽象化トレイト。
///
/// このトレイトは、リングバッファを使用したキューの基本的な操作を定義します。
/// 具体的な実装は、異なる同期機構や永続化戦略を使用できます。
///
/// # 型パラメータ
///
/// * `E` - キューに格納される要素の型
pub trait RingBackend<E> {
  /// キューに要素を追加します。
  ///
  /// # 引数
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 要素の追加が成功した場合
  /// * `Err(QueueError<E>)` - キューが満杯の場合やその他のエラーが発生した場合
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;

  /// キューから要素を取り出します。
  ///
  /// # 戻り値
  ///
  /// * `Ok(Some(E))` - 要素が正常に取り出された場合
  /// * `Ok(None)` - キューが空の場合
  /// * `Err(QueueError<E>)` - エラーが発生した場合
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;

  /// キューの内部状態をクリーンアップします。
  ///
  /// このメソッドは、不要になったリソースの解放や、
  /// 内部バッファの最適化などの保守作業を実行します。
  fn clean_up(&self);

  /// キューに現在格納されている要素の数を返します。
  ///
  /// # 戻り値
  ///
  /// キューのサイズ（`QueueSize::Limited(n)` または `QueueSize::Unlimited`）
  fn len(&self) -> QueueSize;

  /// キューの容量（最大格納可能数）を返します。
  ///
  /// # 戻り値
  ///
  /// キューの容量（`QueueSize::Limited(n)` または `QueueSize::Unlimited`）
  fn capacity(&self) -> QueueSize;

  /// キューの動的サイズ変更機能を有効または無効にします。
  ///
  /// # 引数
  ///
  /// * `dynamic` - `true`の場合、動的サイズ変更を有効化。`false`の場合、無効化。
  fn set_dynamic(&self, dynamic: bool);

  /// キューが空かどうかを判定します。
  ///
  /// # 戻り値
  ///
  /// * `true` - キューが空の場合
  /// * `false` - キューに1つ以上の要素がある場合
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}

/// [`RingBackend`]への参照を提供するハンドルトレイト。
///
/// このトレイトは、リングバッファバックエンドへの共有参照を管理するための
/// ハンドル型を定義します。クローン可能であり、複数のスレッドから安全に
/// アクセスできることを保証します。
///
/// # 型パラメータ
///
/// * `E` - キューに格納される要素の型
pub trait RingHandle<E>: Shared<Self::Backend> + Clone {
  /// このハンドルが参照するバックエンドの型。
  ///
  /// `?Sized`により、動的サイズのトレイトオブジェクトも許容します。
  type Backend: RingBackend<E> + ?Sized;

  /// バックエンドへの参照を取得します。
  ///
  /// # 戻り値
  ///
  /// バックエンドへの不変参照
  fn backend(&self) -> &Self::Backend;
}

/// リングバッファのストレージハンドルを直接操作するバックエンド実装。
///
/// この構造体は、[`QueueHandle`]を介してリングバッファストレージに
/// 直接アクセスする具体的なバックエンド実装を提供します。
///
/// # 型パラメータ
///
/// * `S` - リングバッファのストレージハンドル型
///
/// [`QueueHandle`]: crate::collections::queue::QueueHandle
#[derive(Debug)]
pub struct RingStorageBackend<S> {
  storage: S,
}

impl<S> RingStorageBackend<S> {
  /// 新しい`RingStorageBackend`を作成します。
  ///
  /// # 引数
  ///
  /// * `storage` - 使用するストレージハンドル
  ///
  /// # 戻り値
  ///
  /// 新しい`RingStorageBackend`インスタンス
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// ストレージハンドルへの参照を取得します。
  ///
  /// # 戻り値
  ///
  /// ストレージハンドルへの不変参照
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// このバックエンドを消費し、内部のストレージハンドルを返します。
  ///
  /// # 戻り値
  ///
  /// 内部のストレージハンドル
  pub fn into_storage(self) -> S {
    self.storage
  }
}

/// `RingStorageBackend`の[`RingBackend`]トレイト実装。
///
/// この実装は、ストレージハンドルを介してリングバッファに対する
/// すべての操作を委譲します。各操作は適切な読み取りまたは書き込みロックを
/// 使用して実行されます。
impl<S, E> RingBackend<E> for RingStorageBackend<S>
where
  S: crate::collections::queue::QueueHandle<E>,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.storage.storage().with_write(|buffer| buffer.offer_mut(element))
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.storage.storage().with_write(|buffer| buffer.poll_mut())
  }

  fn clean_up(&self) {
    self.storage.storage().with_write(|buffer| buffer.clean_up_mut());
  }

  fn len(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.capacity())
  }

  fn set_dynamic(&self, dynamic: bool) {
    self.storage.storage().with_write(|buffer| buffer.set_dynamic(dynamic));
  }
}
