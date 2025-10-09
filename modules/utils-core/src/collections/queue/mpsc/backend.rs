use super::super::storage::RingBufferStorage;
use super::traits::MpscBackend;
use crate::collections::{QueueError, QueueSize};

/// リングバッファストレージを使用して共有マルチプロデューサー/シングルコンシューマー
/// キューを駆動するバックエンド実装。
///
/// このバックエンドは、インメモリの[`RingBufferStorage`]を利用して、
/// 複数のプロデューサーから単一のコンシューマーへの効率的なメッセージ配送を実現します。
#[derive(Debug)]
pub struct RingBufferBackend<S> {
  storage: S,
}

impl<S> RingBufferBackend<S> {
  /// 指定されたストレージを使用して新しい`RingBufferBackend`を作成します。
  ///
  /// # Arguments
  ///
  /// * `storage` - バックエンドが使用するリングバッファストレージ
  ///
  /// # Returns
  ///
  /// 新しい`RingBufferBackend`インスタンス
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// 内部ストレージへの不変参照を返します。
  ///
  /// # Returns
  ///
  /// ストレージへの参照
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// 内部ストレージの所有権を取得して返します。
  ///
  /// このメソッドはバックエンドを消費し、内部ストレージを返します。
  ///
  /// # Returns
  ///
  /// 内部ストレージの所有権
  pub fn into_storage(self) -> S {
    self.storage
  }
}

impl<S, T> MpscBackend<T> for RingBufferBackend<S>
where
  S: RingBufferStorage<T>,
{
  /// キューに要素を送信しようと試みます。
  ///
  /// このメソッドはブロッキングせずに要素をキューに追加しようとします。
  /// キューが満杯の場合やクローズされている場合はエラーを返します。
  ///
  /// # Arguments
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 要素が正常にキューに追加された場合
  /// * `Err(QueueError<T>)` - キューが満杯またはクローズされている場合
  fn try_send(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.with_write(|buffer| buffer.offer(element))
  }

  /// キューから要素を受信しようと試みます。
  ///
  /// このメソッドはブロッキングせずにキューから要素を取得しようとします。
  /// キューが空の場合は`None`を返します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(T))` - 要素が正常に取得された場合
  /// * `Ok(None)` - キューが空の場合
  /// * `Err(QueueError<T>)` - エラーが発生した場合
  fn try_recv(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.with_write(|buffer| buffer.poll())
  }

  /// キューをクローズし、リソースをクリーンアップします。
  ///
  /// このメソッドを呼び出すと、キューへの新しい送信はできなくなります。
  /// 既にキューに存在する要素は引き続き受信可能です。
  fn close(&self) {
    self.storage.with_write(|buffer| buffer.clean_up());
  }

  /// キュー内の現在の要素数を返します。
  ///
  /// # Returns
  ///
  /// キュー内の要素数を表す[`QueueSize`]
  fn len(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.len())
  }

  /// キューの容量を返します。
  ///
  /// # Returns
  ///
  /// キューの容量を表す[`QueueSize`]（無制限の場合は`QueueSize::Unbounded`）
  fn capacity(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.capacity())
  }

  /// キューがクローズされているかどうかを確認します。
  ///
  /// # Returns
  ///
  /// * `true` - キューがクローズされている場合
  /// * `false` - キューがオープンしている場合
  fn is_closed(&self) -> bool {
    self.storage.with_read(|buffer| buffer.is_closed())
  }

  /// キューの容量を設定します。
  ///
  /// このメソッドはキューの最大容量を変更します。
  ///
  /// # Arguments
  ///
  /// * `capacity` - 新しい容量（`None`の場合は無制限）
  ///
  /// # Returns
  ///
  /// * `true` - 容量が正常に設定された場合
  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.with_write(|buffer| buffer.set_capacity(capacity));
    true
  }
}
