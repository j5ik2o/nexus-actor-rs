use crate::collections::{QueueError, QueueSize};
use crate::sync::Shared;

/// MPSCキューのバックエンドを抽象化するトランスポート指向のトレイトです。
///
/// このトレイトは、Multiple Producer Single Consumer (MPSC) キューの基本的な操作を定義します。
/// 複数の送信者と単一の受信者の間でメッセージを非同期に伝達するための抽象インターフェースです。
pub trait MpscBackend<T> {
  /// 要素をキューに送信を試みます（ノンブロッキング）。
  ///
  /// # Arguments
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 要素が正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯、クローズ済み、またはその他のエラーが発生した場合
  fn try_send(&self, element: T) -> Result<(), QueueError<T>>;

  /// キューから要素を受信を試みます（ノンブロッキング）。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - 要素が正常に受信された場合
  /// * `Ok(None)` - キューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  fn try_recv(&self) -> Result<Option<T>, QueueError<T>>;

  /// キューをクローズします。
  ///
  /// クローズ後は新しい要素の送信ができなくなりますが、
  /// 既にキューに入っている要素は受信可能です。
  fn close(&self);

  /// キューに現在格納されている要素数を取得します。
  ///
  /// # Returns
  ///
  /// 要素数を表す `QueueSize`（有界または無界）
  fn len(&self) -> QueueSize;

  /// キューの容量を取得します。
  ///
  /// # Returns
  ///
  /// 容量を表す `QueueSize`（有界または無界）
  fn capacity(&self) -> QueueSize;

  /// キューがクローズされているかどうかを確認します。
  ///
  /// # Returns
  ///
  /// * `true` - キューがクローズされている場合
  /// * `false` - キューがオープン状態の場合
  fn is_closed(&self) -> bool;

  /// キューの容量を設定します。
  ///
  /// デフォルト実装では何も行わず、`false` を返します。
  /// バックエンド実装によってはこのメソッドをオーバーライドして動的な容量変更をサポートできます。
  ///
  /// # Arguments
  ///
  /// * `capacity` - 新しい容量。`None` の場合は無制限
  ///
  /// # Returns
  ///
  /// * `true` - 容量の設定が成功した場合
  /// * `false` - 容量の設定がサポートされていない、または失敗した場合
  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    let _ = capacity;
    false
  }

  /// キューが空かどうかを確認します。
  ///
  /// # Returns
  ///
  /// * `true` - キューが空の場合
  /// * `false` - キューに1つ以上の要素がある場合
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}

/// [`MpscBackend`] を公開する共有ハンドルトレイトです。
///
/// このトレイトは、複数のスレッドから安全にアクセスできる共有ハンドルを表します。
/// `Shared` および `Clone` を実装することで、ハンドルを複数の送信者間で共有可能にします。
pub trait MpscHandle<T>: Shared<Self::Backend> + Clone {
  /// このハンドルが使用するバックエンドの型です。
  ///
  /// バックエンドは [`MpscBackend`] トレイトを実装している必要があります。
  /// `?Sized` により、動的サイズのバックエンド（トレイトオブジェクトなど）もサポートします。
  type Backend: MpscBackend<T> + ?Sized;

  /// このハンドルが管理するバックエンドへの参照を取得します。
  ///
  /// # Returns
  ///
  /// バックエンドへの参照
  fn backend(&self) -> &Self::Backend;
}
