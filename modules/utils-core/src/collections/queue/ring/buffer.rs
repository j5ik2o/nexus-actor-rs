use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::mem::MaybeUninit;

use crate::collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

/// リングバッファのデフォルト容量
///
/// リングバッファが作成される際の初期容量として使用されます。
pub const DEFAULT_CAPACITY: usize = 32;

/// リングバッファの実装
///
/// FIFOキューとして動作する循環バッファです。
/// 容量に達した際の動作は`dynamic`フラグにより制御されます：
/// - `dynamic = true`: 容量が自動的に2倍に拡張されます
/// - `dynamic = false`: 新しい要素の追加は`QueueError::Full`エラーを返します
///
/// # 型パラメータ
///
/// * `T` - バッファに格納される要素の型
#[derive(Debug)]
pub struct RingBuffer<T> {
  /// 内部バッファ（未初期化メモリを含む可能性があります）
  buf: Box<[MaybeUninit<T>]>,
  /// 読み取り位置を示すヘッドインデックス
  head: usize,
  /// 書き込み位置を示すテールインデックス
  tail: usize,
  /// 現在のバッファ内の要素数
  len: usize,
  /// 容量の動的拡張を有効にするかどうか
  dynamic: bool,
}

impl<T> RingBuffer<T> {
  /// 指定された容量で新しいリングバッファを作成します
  ///
  /// デフォルトでは動的拡張が有効になっています（`dynamic = true`）。
  ///
  /// # パラメータ
  ///
  /// * `capacity` - 初期容量（0より大きい値を指定する必要があります）
  ///
  /// # Panics
  ///
  /// * `capacity`が0の場合にパニックします
  ///
  /// # 例
  ///
  /// ```
  /// # use nexus_actor_utils_rs::collections::queue::ring::RingBuffer;
  /// let buffer = RingBuffer::<i32>::new(10);
  /// ```
  pub fn new(capacity: usize) -> Self {
    assert!(capacity > 0, "capacity must be > 0");
    let buf = Self::alloc_buffer(capacity);
    Self {
      buf,
      head: 0,
      tail: 0,
      len: 0,
      dynamic: true,
    }
  }

  /// 動的拡張の有効/無効を設定してリングバッファを返します（ビルダーパターン）
  ///
  /// # パラメータ
  ///
  /// * `dynamic` - `true`の場合、容量に達したときに自動的に拡張されます
  ///
  /// # 例
  ///
  /// ```
  /// # use nexus_actor_utils_rs::collections::queue::ring::RingBuffer;
  /// let buffer = RingBuffer::<i32>::new(10).with_dynamic(false);
  /// ```
  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.dynamic = dynamic;
    self
  }

  /// 動的拡張の有効/無効を設定します
  ///
  /// # パラメータ
  ///
  /// * `dynamic` - `true`の場合、容量に達したときに自動的に拡張されます
  pub fn set_dynamic(&mut self, dynamic: bool) {
    self.dynamic = dynamic;
  }

  /// 指定された容量で新しいバッファを割り当てます
  ///
  /// # パラメータ
  ///
  /// * `capacity` - 割り当てる容量
  ///
  /// # Returns
  ///
  /// 未初期化メモリを含むボックス化されたスライス
  fn alloc_buffer(capacity: usize) -> Box<[MaybeUninit<T>]> {
    let mut vec = Vec::with_capacity(capacity);
    vec.resize_with(capacity, MaybeUninit::uninit);
    vec.into_boxed_slice()
  }

  /// バッファの容量を2倍に拡張します
  ///
  /// 既存の要素をすべて取り出し、新しいバッファに再挿入します。
  /// この操作中に既存の要素は保持されます。
  ///
  /// # Panics
  ///
  /// 再挿入時に容量不足が発生した場合にパニックします
  /// （通常は発生しないはずです）
  fn grow(&mut self) {
    let new_cap = self.buf.len().saturating_mul(2).max(1);
    let mut items = Vec::with_capacity(self.len);
    while let Ok(Some(item)) = self.poll_mut() {
      items.push(item);
    }

    self.buf = Self::alloc_buffer(new_cap);
    self.head = 0;
    self.tail = 0;
    self.len = 0;

    for item in items {
      // reinsert without triggering another grow
      if let Err(QueueError::Full(_)) = self.offer_mut(item) {
        panic!("ring buffer grow failed to reserve capacity");
      }
    }
  }
}

impl<T> QueueBase<T> for RingBuffer<T> {
  /// バッファ内の現在の要素数を返します
  ///
  /// # Returns
  ///
  /// 要素数を表す`QueueSize::limited`
  fn len(&self) -> QueueSize {
    QueueSize::limited(self.len)
  }

  /// バッファの容量を返します
  ///
  /// # Returns
  ///
  /// * `dynamic = true`の場合: `QueueSize::limitless()`（無制限）
  /// * `dynamic = false`の場合: `QueueSize::limited(capacity)`（制限あり）
  fn capacity(&self) -> QueueSize {
    if self.dynamic {
      QueueSize::limitless()
    } else {
      QueueSize::limited(self.buf.len())
    }
  }
}

impl<T> QueueWriter<T> for RingBuffer<T> {
  /// バッファに要素を追加します
  ///
  /// バッファが満杯の場合：
  /// * `dynamic = true`: 容量を2倍に拡張してから追加します
  /// * `dynamic = false`: `QueueError::Full`を返します
  ///
  /// # パラメータ
  ///
  /// * `item` - 追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 追加に成功した場合
  /// * `Err(QueueError::Full(item))` - バッファが満杯で動的拡張が無効な場合
  ///
  /// # 例
  ///
  /// ```
  /// # use nexus_actor_utils_rs::collections::queue::{QueueWriter, ring::RingBuffer};
  /// let mut buffer = RingBuffer::new(2).with_dynamic(false);
  /// buffer.offer_mut(1).unwrap();
  /// buffer.offer_mut(2).unwrap();
  /// assert!(buffer.offer_mut(3).is_err()); // 満杯のためエラー
  /// ```
  fn offer_mut(&mut self, item: T) -> Result<(), QueueError<T>> {
    if self.len == self.buf.len() {
      if self.dynamic {
        self.grow();
      } else {
        return Err(QueueError::Full(item));
      }
    }

    self.buf[self.tail] = MaybeUninit::new(item);
    self.tail = (self.tail + 1) % self.buf.len();
    self.len += 1;
    Ok(())
  }
}

impl<T> QueueReader<T> for RingBuffer<T> {
  /// バッファから要素を取り出します（FIFO順）
  ///
  /// # Returns
  ///
  /// * `Ok(Some(item))` - 要素が取り出された場合
  /// * `Ok(None)` - バッファが空の場合
  ///
  /// # 例
  ///
  /// ```
  /// # use nexus_actor_utils_rs::collections::queue::{QueueWriter, QueueReader, ring::RingBuffer};
  /// let mut buffer = RingBuffer::new(10);
  /// buffer.offer_mut(1).unwrap();
  /// buffer.offer_mut(2).unwrap();
  /// assert_eq!(buffer.poll_mut().unwrap(), Some(1));
  /// assert_eq!(buffer.poll_mut().unwrap(), Some(2));
  /// assert_eq!(buffer.poll_mut().unwrap(), None);
  /// ```
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    if self.len == 0 {
      return Ok(None);
    }

    let value = unsafe { self.buf[self.head].assume_init_read() };
    self.head = (self.head + 1) % self.buf.len();
    self.len -= 1;
    Ok(Some(value))
  }

  /// バッファ内のすべての要素を破棄します
  ///
  /// すべての要素を順番に取り出してドロップします。
  /// この操作後、バッファは空になります。
  fn clean_up_mut(&mut self) {
    while self.len > 0 {
      let _ = self.poll_mut();
    }
  }
}

impl<T> Default for RingBuffer<T> {
  /// デフォルトのリングバッファを作成します
  ///
  /// `DEFAULT_CAPACITY`（32）の容量で、動的拡張が有効なバッファを作成します。
  ///
  /// # 例
  ///
  /// ```
  /// # use nexus_actor_utils_rs::collections::queue::ring::RingBuffer;
  /// let buffer = RingBuffer::<i32>::default();
  /// ```
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;

  #[test]
  fn ring_buffer_offer_poll() {
    let mut buffer = RingBuffer::new(2).with_dynamic(false);
    buffer.offer_mut(1).unwrap();
    buffer.offer_mut(2).unwrap();
    assert_eq!(buffer.offer_mut(3), Err(QueueError::Full(3)));

    assert_eq!(buffer.poll_mut().unwrap(), Some(1));
    assert_eq!(buffer.poll_mut().unwrap(), Some(2));
    assert_eq!(buffer.poll_mut().unwrap(), None);
  }

  #[test]
  fn ring_buffer_grows_when_dynamic() {
    let mut buffer = RingBuffer::new(1);
    buffer.offer_mut(1).unwrap();
    buffer.offer_mut(2).unwrap();
    assert_eq!(buffer.len().to_usize(), 2);
    assert!(buffer.capacity().is_limitless());
  }
}
