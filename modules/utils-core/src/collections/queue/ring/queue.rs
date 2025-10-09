use core::marker::PhantomData;

use super::backend::{RingBackend, RingHandle};
use crate::collections::queue::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter};

/// Queue facade that delegates all operations to a [`RingBackend`].
#[derive(Debug)]
pub struct RingQueue<H, E>
where
  H: RingHandle<E>, {
  backend: H,
  _marker: PhantomData<E>,
}

impl<H, E> RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// 指定されたバックエンドハンドルから新しい`RingQueue`を作成します。
  ///
  /// # 引数
  ///
  /// * `backend` - リングキューのバックエンドハンドル
  ///
  /// # 戻り値
  ///
  /// 新しい`RingQueue`インスタンス
  pub fn new(backend: H) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  /// バックエンドハンドルへの参照を返します。
  ///
  /// # 戻り値
  ///
  /// バックエンドハンドルへの不変参照
  pub fn backend(&self) -> &H {
    &self.backend
  }

  /// このキューを消費し、内部のバックエンドハンドルを返します。
  ///
  /// # 戻り値
  ///
  /// 内部のバックエンドハンドル
  pub fn into_backend(self) -> H {
    self.backend
  }

  /// キューの動的モードを設定します。
  ///
  /// 動的モードが有効な場合、キューは容量を超えた要素の追加時に自動的に拡張されます。
  ///
  /// # 引数
  ///
  /// * `dynamic` - `true`の場合、動的モードを有効にします
  pub fn set_dynamic(&self, dynamic: bool) {
    self.backend.backend().set_dynamic(dynamic);
  }

  /// 動的モードを設定し、このキューを返すビルダーメソッド。
  ///
  /// 動的モードが有効な場合、キューは容量を超えた要素の追加時に自動的に拡張されます。
  ///
  /// # 引数
  ///
  /// * `dynamic` - `true`の場合、動的モードを有効にします
  ///
  /// # 戻り値
  ///
  /// 動的モードが設定されたこのキュー
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// キューに要素を追加します。
  ///
  /// # 引数
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 要素が正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯で要素を追加できない場合
  ///
  /// # エラー
  ///
  /// キューが満杯で動的モードが無効な場合、`QueueError::Full`が返されます。
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.backend.backend().offer(element)
  }

  /// キューから要素を取り出します。
  ///
  /// # 戻り値
  ///
  /// * `Ok(Some(E))` - キューから取り出された要素
  /// * `Ok(None)` - キューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.backend.backend().poll()
  }

  /// キューのクリーンアップを実行します。
  ///
  /// 内部バッファのメモリ整理などを行います。
  pub fn clean_up(&self) {
    self.backend.backend().clean_up();
  }
}

impl<H, E> Clone for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// `RingQueue`のクローンを作成します。
  ///
  /// バックエンドハンドルをクローンし、新しい`RingQueue`インスタンスを返します。
  ///
  /// # 戻り値
  ///
  /// このキューのクローン
  fn clone(&self) -> Self {
    Self {
      backend: self.backend.clone(),
      _marker: PhantomData,
    }
  }
}

impl<H, E> QueueBase<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// キュー内の要素数を返します。
  ///
  /// # 戻り値
  ///
  /// キュー内の現在の要素数
  fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  /// キューの容量を返します。
  ///
  /// # 戻り値
  ///
  /// キューが保持できる最大要素数
  fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, E> QueueWriter<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// 可変参照を使用してキューに要素を追加します。
  ///
  /// # 引数
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 要素が正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯で要素を追加できない場合
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<H, E> QueueReader<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// 可変参照を使用してキューから要素を取り出します。
  ///
  /// # 戻り値
  ///
  /// * `Ok(Some(E))` - キューから取り出された要素
  /// * `Ok(None)` - キューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// 可変参照を使用してキューのクリーンアップを実行します。
  ///
  /// 内部バッファのメモリ整理などを行います。
  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<H, E> QueueRw<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// 共有参照を使用してキューに要素を追加します。
  ///
  /// # 引数
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 要素が正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯で要素を追加できない場合
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  /// 共有参照を使用してキューから要素を取り出します。
  ///
  /// # 戻り値
  ///
  /// * `Ok(Some(E))` - キューから取り出された要素
  /// * `Ok(None)` - キューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// 共有参照を使用してキューのクリーンアップを実行します。
  ///
  /// 内部バッファのメモリ整理などを行います。
  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::super::{RingBuffer, RingStorageBackend};
  use super::RingQueue;
  use crate::collections::queue::ring::backend::RingHandle;
  use crate::collections::queue::QueueHandle;

  struct RcStorageHandle<E>(Rc<RefCell<RingBuffer<E>>>);

  impl<E> Clone for RcStorageHandle<E> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<E> core::ops::Deref for RcStorageHandle<E> {
    type Target = RefCell<RingBuffer<E>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<E> QueueHandle<E> for RcStorageHandle<E> {
    type Storage = RefCell<RingBuffer<E>>;

    fn storage(&self) -> &Self::Storage {
      &self.0
    }
  }

  impl<E> crate::sync::Shared<RefCell<RingBuffer<E>>> for RcStorageHandle<E> {}

  struct RcBackendHandle<E>(Rc<RingStorageBackend<RcStorageHandle<E>>>);

  impl<E> Clone for RcBackendHandle<E> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<E> core::ops::Deref for RcBackendHandle<E> {
    type Target = RingStorageBackend<RcStorageHandle<E>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<E> crate::sync::Shared<RingStorageBackend<RcStorageHandle<E>>> for RcBackendHandle<E> {}

  impl<E> RingHandle<E> for RcBackendHandle<E> {
    type Backend = RingStorageBackend<RcStorageHandle<E>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn shared_ring_queue_offer_poll() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(RingBuffer::new(2))));
    let backend = RcBackendHandle(Rc::new(RingStorageBackend::new(storage)));
    let queue = RingQueue::new(backend);

    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }
}
