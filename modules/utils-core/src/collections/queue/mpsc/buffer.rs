use alloc::collections::VecDeque;

use crate::collections::{QueueError, QueueSize};

/// マルチプロデューサー・シングルコンシューマー（MPSC）用のバッファ実装。
///
/// このバッファは、複数の送信者と単一の受信者の間でメッセージをやり取りするために使用されます。
/// 容量制限の有無を選択でき、クローズ状態の管理もサポートします。
///
/// # 特徴
///
/// - 有界・無界の両方をサポート
/// - クローズ状態の管理
/// - スレッドセーフ（適切なロック機構と組み合わせて使用）
///
/// # 例
///
/// ```
/// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
///
/// let mut buffer = MpscBuffer::new(Some(10));
/// assert!(buffer.offer(42).is_ok());
/// assert_eq!(buffer.poll().unwrap(), Some(42));
/// ```
#[derive(Debug, Clone)]
pub struct MpscBuffer<T> {
  buffer: VecDeque<T>,
  capacity: Option<usize>,
  closed: bool,
}

impl<T> MpscBuffer<T> {
  /// 新しい`MpscBuffer`を作成します。
  ///
  /// # 引数
  ///
  /// * `capacity` - バッファの容量。`None`の場合は無制限。
  ///
  /// # 戻り値
  ///
  /// 新しい`MpscBuffer`インスタンス。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// // 容量10の有界バッファ
  /// let bounded = MpscBuffer::<i32>::new(Some(10));
  ///
  /// // 無制限のバッファ
  /// let unbounded = MpscBuffer::<i32>::new(None);
  /// ```
  pub fn new(capacity: Option<usize>) -> Self {
    Self {
      buffer: VecDeque::new(),
      capacity,
      closed: false,
    }
  }

  /// バッファ内の現在の要素数を返します。
  ///
  /// # 戻り値
  ///
  /// 現在の要素数を表す`QueueSize`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  /// buffer.offer(2).unwrap();
  /// assert_eq!(buffer.len().value(), Some(2));
  /// ```
  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.buffer.len())
  }

  /// バッファの容量を返します。
  ///
  /// # 戻り値
  ///
  /// バッファの容量を表す`QueueSize`。無制限の場合は`QueueSize::limitless()`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// let bounded = MpscBuffer::<i32>::new(Some(10));
  /// assert_eq!(bounded.capacity().value(), Some(10));
  ///
  /// let unbounded = MpscBuffer::<i32>::new(None);
  /// assert!(unbounded.capacity().is_limitless());
  /// ```
  pub fn capacity(&self) -> QueueSize {
    match self.capacity {
      Some(limit) => QueueSize::limited(limit),
      None => QueueSize::limitless(),
    }
  }

  /// バッファの容量を設定します。
  ///
  /// 新しい容量が現在のバッファサイズより小さい場合、バッファは切り詰められます。
  ///
  /// # 引数
  ///
  /// * `capacity` - 新しい容量。`None`の場合は無制限。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  /// buffer.offer(2).unwrap();
  /// buffer.offer(3).unwrap();
  ///
  /// // 容量を2に縮小（3番目の要素は削除される）
  /// buffer.set_capacity(Some(2));
  /// assert_eq!(buffer.len().value(), Some(2));
  /// ```
  pub fn set_capacity(&mut self, capacity: Option<usize>) {
    self.capacity = capacity;
    if let Some(limit) = capacity {
      if self.buffer.len() > limit {
        self.buffer.truncate(limit);
      }
    }
  }

  /// バッファに要素を追加します。
  ///
  /// バッファがクローズされている場合や容量が満杯の場合はエラーを返します。
  ///
  /// # 引数
  ///
  /// * `element` - 追加する要素。
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 要素の追加に成功。
  /// * `Err(QueueError::Closed(element))` - バッファがクローズされている。
  /// * `Err(QueueError::Full(element))` - バッファが満杯。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  /// use nexus_utils_core_rs::collections::QueueError;
  ///
  /// let mut buffer = MpscBuffer::new(Some(2));
  /// assert!(buffer.offer(1).is_ok());
  /// assert!(buffer.offer(2).is_ok());
  ///
  /// // 容量超過
  /// assert!(matches!(buffer.offer(3), Err(QueueError::Full(_))));
  /// ```
  pub fn offer(&mut self, element: T) -> Result<(), QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(element));
    }
    if matches!(self.capacity, Some(limit) if self.buffer.len() >= limit) {
      return Err(QueueError::Full(element));
    }
    self.buffer.push_back(element);
    Ok(())
  }

  /// バッファから要素を取り出します。
  ///
  /// バッファが空でクローズされている場合は`Disconnected`エラーを返します。
  ///
  /// # 戻り値
  ///
  /// * `Ok(Some(element))` - 要素の取り出しに成功。
  /// * `Ok(None)` - バッファが空（まだクローズされていない）。
  /// * `Err(QueueError::Disconnected)` - バッファが空でクローズされている。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  /// use nexus_utils_core_rs::collections::QueueError;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(42).unwrap();
  ///
  /// assert_eq!(buffer.poll().unwrap(), Some(42));
  /// assert_eq!(buffer.poll().unwrap(), None);
  ///
  /// buffer.mark_closed();
  /// assert!(matches!(buffer.poll(), Err(QueueError::Disconnected)));
  /// ```
  pub fn poll(&mut self) -> Result<Option<T>, QueueError<T>> {
    if let Some(item) = self.buffer.pop_front() {
      return Ok(Some(item));
    }
    if self.closed {
      Err(QueueError::Disconnected)
    } else {
      Ok(None)
    }
  }

  /// バッファをクリーンアップし、クローズ状態にします。
  ///
  /// すべての要素を削除し、バッファをクローズ状態にマークします。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  /// buffer.offer(2).unwrap();
  ///
  /// buffer.clean_up();
  /// assert!(buffer.is_closed());
  /// assert_eq!(buffer.len().value(), Some(0));
  /// ```
  pub fn clean_up(&mut self) {
    self.buffer.clear();
    self.closed = true;
  }

  /// バッファがクローズされているかどうかを返します。
  ///
  /// # 戻り値
  ///
  /// バッファがクローズされている場合は`true`、それ以外は`false`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::<i32>::new(Some(10));
  /// assert!(!buffer.is_closed());
  ///
  /// buffer.mark_closed();
  /// assert!(buffer.is_closed());
  /// ```
  pub fn is_closed(&self) -> bool {
    self.closed
  }

  /// バッファをクローズ状態にマークします。
  ///
  /// 既存の要素は削除されません。新しい要素の追加のみが禁止されます。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  /// use nexus_utils_core_rs::collections::QueueError;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  ///
  /// buffer.mark_closed();
  /// assert!(buffer.is_closed());
  ///
  /// // クローズ後は要素を追加できない
  /// assert!(matches!(buffer.offer(2), Err(QueueError::Closed(_))));
  ///
  /// // しかし既存の要素は取り出せる
  /// assert_eq!(buffer.poll().unwrap(), Some(1));
  /// ```
  pub fn mark_closed(&mut self) {
    self.closed = true;
  }
}

impl<T> Default for MpscBuffer<T> {
  /// デフォルトの`MpscBuffer`を作成します。
  ///
  /// 容量無制限のバッファを返します。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::collections::queue::mpsc::MpscBuffer;
  ///
  /// let buffer: MpscBuffer<i32> = Default::default();
  /// assert!(buffer.capacity().is_limitless());
  /// ```
  fn default() -> Self {
    Self::new(None)
  }
}
