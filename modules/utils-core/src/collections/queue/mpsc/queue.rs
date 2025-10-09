use super::traits::{MpscBackend, MpscHandle};
use crate::collections::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter};

/// [`MpscBackend`]を操作するキューファサード
///
/// このキューは、マルチプロデューサー・シングルコンシューマー（MPSC）パターンを実装しており、
/// 複数のスレッドから要素を追加し、単一のスレッドから要素を取り出すことができます。
#[derive(Debug)]
pub struct MpscQueue<S, T>
where
  S: MpscHandle<T>, {
  storage: S,
  _marker: core::marker::PhantomData<T>,
}

impl<S, T> MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// 指定されたストレージを使用して新しい[`MpscQueue`]を作成します。
  ///
  /// # Arguments
  ///
  /// * `storage` - キューのバックエンドストレージ
  ///
  /// # Returns
  ///
  /// 新しい[`MpscQueue`]インスタンス
  pub fn new(storage: S) -> Self {
    Self {
      storage,
      _marker: core::marker::PhantomData,
    }
  }

  /// バックエンドストレージへの参照を取得します。
  ///
  /// # Returns
  ///
  /// ストレージへの不変参照
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// キューからバックエンドストレージを取り出します。
  ///
  /// このメソッドは[`MpscQueue`]を消費し、所有権をストレージに移します。
  ///
  /// # Returns
  ///
  /// バックエンドストレージ
  pub fn into_storage(self) -> S {
    self.storage
  }

  /// キューの容量を設定します。
  ///
  /// # Arguments
  ///
  /// * `capacity` - 新しい容量。`None`は無制限を意味します。
  ///
  /// # Returns
  ///
  /// 容量の設定に成功した場合は`true`、失敗した場合は`false`
  pub fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.backend().set_capacity(capacity)
  }

  /// 要素をキューに追加します。
  ///
  /// キューが満杯の場合、または閉じている場合はエラーを返します。
  ///
  /// # Arguments
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 要素の追加に成功
  /// * `Err(QueueError::Full(element))` - キューが満杯
  /// * `Err(QueueError::Closed(element))` - キューが閉じている
  pub fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.backend().try_send(element)
  }

  /// キューから要素を取り出します。
  ///
  /// キューが空の場合は`None`を返します。キューが閉じている場合はエラーを返します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - 要素の取り出しに成功
  /// * `Ok(None)` - キューが空
  /// * `Err(QueueError::Disconnected)` - キューが閉じている
  pub fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.backend().try_recv()
  }

  /// キューをクリーンアップし、閉じます。
  ///
  /// このメソッドを呼び出すと、以降の`offer`操作は失敗し、
  /// `poll`操作は残りの要素を取り出した後にエラーを返します。
  pub fn clean_up(&self) {
    self.storage.backend().close();
  }

  /// キューの容量を取得します。
  ///
  /// # Returns
  ///
  /// キューの容量。無制限の場合は[`QueueSize::Unbounded`]
  pub fn capacity(&self) -> QueueSize {
    self.storage.backend().capacity()
  }

  /// キューが閉じているかどうかを確認します。
  ///
  /// # Returns
  ///
  /// キューが閉じている場合は`true`、そうでない場合は`false`
  pub fn is_closed(&self) -> bool {
    self.storage.backend().is_closed()
  }

  /// バックエンドへの参照を取得します（内部使用）。
  ///
  /// # Returns
  ///
  /// バックエンドへの参照
  fn backend(&self) -> &S::Backend {
    self.storage.backend()
  }
}

impl<S, T> Clone for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// キューのクローンを作成します。
  ///
  /// バックエンドストレージは共有されるため、クローンされたキューは
  /// 同じキューインスタンスを参照します。
  ///
  /// # Returns
  ///
  /// 同じバックエンドストレージを共有する新しい[`MpscQueue`]インスタンス
  fn clone(&self) -> Self {
    Self {
      storage: self.storage.clone(),
      _marker: core::marker::PhantomData,
    }
  }
}

impl<S, T> QueueBase<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// キュー内の要素数を取得します。
  ///
  /// # Returns
  ///
  /// キュー内の要素数。無制限の場合は[`QueueSize::Unbounded`]
  fn len(&self) -> QueueSize {
    self.backend().len()
  }

  /// キューの容量を取得します。
  ///
  /// # Returns
  ///
  /// キューの容量。無制限の場合は[`QueueSize::Unbounded`]
  fn capacity(&self) -> QueueSize {
    self.capacity()
  }
}

impl<S, T> QueueWriter<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// 可変参照を使用して要素をキューに追加します。
  ///
  /// キューが満杯の場合、または閉じている場合はエラーを返します。
  ///
  /// # Arguments
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 要素の追加に成功
  /// * `Err(QueueError::Full(element))` - キューが満杯
  /// * `Err(QueueError::Closed(element))` - キューが閉じている
  fn offer_mut(&mut self, element: T) -> Result<(), QueueError<T>> {
    self.backend().try_send(element)
  }
}

impl<S, T> QueueReader<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// 可変参照を使用してキューから要素を取り出します。
  ///
  /// キューが空の場合は`None`を返します。キューが閉じている場合はエラーを返します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - 要素の取り出しに成功
  /// * `Ok(None)` - キューが空
  /// * `Err(QueueError::Disconnected)` - キューが閉じている
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    self.backend().try_recv()
  }

  /// 可変参照を使用してキューをクリーンアップし、閉じます。
  ///
  /// このメソッドを呼び出すと、以降の`offer_mut`操作は失敗し、
  /// `poll_mut`操作は残りの要素を取り出した後にエラーを返します。
  fn clean_up_mut(&mut self) {
    self.backend().close();
  }
}

impl<S, T> QueueRw<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// 共有参照を使用して要素をキューに追加します。
  ///
  /// キューが満杯の場合、または閉じている場合はエラーを返します。
  ///
  /// # Arguments
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 要素の追加に成功
  /// * `Err(QueueError::Full(element))` - キューが満杯
  /// * `Err(QueueError::Closed(element))` - キューが閉じている
  fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.offer(element)
  }

  /// 共有参照を使用してキューから要素を取り出します。
  ///
  /// キューが空の場合は`None`を返します。キューが閉じている場合はエラーを返します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - 要素の取り出しに成功
  /// * `Ok(None)` - キューが空
  /// * `Err(QueueError::Disconnected)` - キューが閉じている
  fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.poll()
  }

  /// 共有参照を使用してキューをクリーンアップし、閉じます。
  ///
  /// このメソッドを呼び出すと、以降の`offer`操作は失敗し、
  /// `poll`操作は残りの要素を取り出した後にエラーを返します。
  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use core::fmt;

  use crate::collections::queue::mpsc::backend::RingBufferBackend;
  use crate::collections::queue::mpsc::traits::MpscHandle;
  use crate::collections::queue::mpsc::{MpscBuffer, MpscQueue};
  use crate::collections::QueueError;

  struct RcBackendHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> RcBackendHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> fmt::Debug for RcBackendHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_struct("RcBackendHandle").finish()
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> crate::sync::Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcBackendHandle<T> {}

  impl<T> MpscHandle<T> for RcBackendHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn buffer_offer_and_poll() {
    let mut buffer: MpscBuffer<u32> = MpscBuffer::new(Some(1));
    assert!(buffer.offer(1).is_ok());
    assert!(matches!(buffer.offer(2), Err(QueueError::Full(2))));
    assert_eq!(buffer.poll().unwrap(), Some(1));
    assert!(buffer.poll().unwrap().is_none());
    buffer.clean_up();
    assert!(matches!(buffer.offer(3), Err(QueueError::Closed(3))));
  }

  #[test]
  fn shared_queue_shared_operations() {
    let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(Some(2)));
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert!(queue.offer(3).is_err());
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
  }

  #[test]
  fn shared_queue_cleanup_marks_closed() {
    let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(None));
    queue.offer(1).unwrap();
    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
  }
}
