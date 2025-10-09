use alloc::vec::Vec;

use crate::collections::{QueueError, QueueSize};

/// スタック操作固有のエラー型。
///
/// スタックバッファの操作中に発生する可能性のあるエラーを表します。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError<T> {
  /// スタックが容量上限に達している状態で要素を追加しようとした場合のエラー。
  ///
  /// # フィールド
  ///
  /// * 追加しようとした値が含まれます。
  Full(T),
}

/// `StackError`から`QueueError`への変換実装。
///
/// スタック固有のエラーを汎用のキューエラーに変換します。
impl<T> From<StackError<T>> for QueueError<T> {
  fn from(err: StackError<T>) -> Self {
    match err {
      StackError::Full(value) => QueueError::Full(value),
    }
  }
}

/// LIFO（後入先出）順で値を格納する所有権を持つスタックバッファ。
///
/// # 概要
///
/// `StackBuffer`は、要素を後入先出（Last-In-First-Out）順で管理するデータ構造です。
/// オプションで容量制限を設定でき、制限を超えた場合は追加操作がエラーを返します。
///
/// # 例
///
/// ```
/// use nexus_utils_core_rs::StackBuffer;
///
/// let mut stack = StackBuffer::new();
/// stack.push(1).unwrap();
/// stack.push(2).unwrap();
/// assert_eq!(stack.pop(), Some(2));
/// assert_eq!(stack.pop(), Some(1));
/// ```
#[derive(Debug, Clone)]
pub struct StackBuffer<T> {
  items: Vec<T>,
  capacity: Option<usize>,
}

impl<T> StackBuffer<T> {
  /// 容量制限のない空の新しい`StackBuffer`を作成します。
  ///
  /// # Returns
  ///
  /// 容量無制限の空のスタックバッファ。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let stack: StackBuffer<i32> = StackBuffer::new();
  /// assert!(stack.is_empty());
  /// ```
  pub fn new() -> Self {
    Self {
      items: Vec::new(),
      capacity: None,
    }
  }

  /// 指定された容量制限を持つ新しい`StackBuffer`を作成します。
  ///
  /// # Arguments
  ///
  /// * `capacity` - スタックの最大容量。
  ///
  /// # Returns
  ///
  /// 指定された容量制限を持つ空のスタックバッファ。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::with_capacity(3);
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.push(3).unwrap();
  /// // 容量上限に達しているため、次のpushはエラーを返します
  /// assert!(stack.push(4).is_err());
  /// ```
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      items: Vec::with_capacity(capacity),
      capacity: Some(capacity),
    }
  }

  /// スタックの容量制限を取得します。
  ///
  /// # Returns
  ///
  /// * 容量制限が設定されている場合は`QueueSize::limited`。
  /// * 容量無制限の場合は`QueueSize::limitless`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let stack = StackBuffer::<i32>::with_capacity(10);
  /// assert_eq!(stack.capacity().to_usize(), 10);
  /// ```
  pub fn capacity(&self) -> QueueSize {
    match self.capacity {
      Some(limit) => QueueSize::limited(limit),
      None => QueueSize::limitless(),
    }
  }

  /// スタックの容量制限を設定します。
  ///
  /// 新しい容量が現在の要素数より少ない場合、スタックは新しい容量に切り詰められます。
  ///
  /// # Arguments
  ///
  /// * `capacity` - 新しい容量制限。`None`の場合は容量無制限になります。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.push(3).unwrap();
  ///
  /// // 容量を2に制限すると、最も古い要素が削除されます
  /// stack.set_capacity(Some(2));
  /// assert_eq!(stack.len().to_usize(), 2);
  /// ```
  pub fn set_capacity(&mut self, capacity: Option<usize>) {
    self.capacity = capacity;
    if let Some(limit) = capacity {
      if self.items.len() > limit {
        self.items.truncate(limit);
      }
    }
  }

  /// スタック内の現在の要素数を取得します。
  ///
  /// # Returns
  ///
  /// 現在の要素数を表す`QueueSize`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert_eq!(stack.len().to_usize(), 2);
  /// ```
  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.items.len())
  }

  /// スタックが空かどうかを確認します。
  ///
  /// # Returns
  ///
  /// * スタックが空の場合は`true`。
  /// * 1つ以上の要素がある場合は`false`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// assert!(stack.is_empty());
  /// stack.push(1).unwrap();
  /// assert!(!stack.is_empty());
  /// ```
  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  /// スタックの先頭に要素を追加します。
  ///
  /// 容量制限が設定されており、スタックが既に満杯の場合は、エラーを返します。
  ///
  /// # Arguments
  ///
  /// * `value` - スタックに追加する値。
  ///
  /// # Returns
  ///
  /// * 成功した場合は`Ok(())`。
  /// * スタックが満杯の場合は`Err(StackError::Full(value))`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::with_capacity(2);
  /// assert!(stack.push(1).is_ok());
  /// assert!(stack.push(2).is_ok());
  /// assert!(stack.push(3).is_err()); // 容量上限
  /// ```
  pub fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    if matches!(self.capacity, Some(limit) if self.items.len() >= limit) {
      return Err(StackError::Full(value));
    }
    self.items.push(value);
    Ok(())
  }

  /// スタックの先頭から要素を取り出します。
  ///
  /// この操作は、最後に追加された要素を削除して返します（LIFO順）。
  ///
  /// # Returns
  ///
  /// * スタックに要素がある場合は`Some(value)`。
  /// * スタックが空の場合は`None`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert_eq!(stack.pop(), Some(2));
  /// assert_eq!(stack.pop(), Some(1));
  /// assert_eq!(stack.pop(), None);
  /// ```
  pub fn pop(&mut self) -> Option<T> {
    self.items.pop()
  }

  /// スタックの先頭要素への参照を取得します（削除はしません）。
  ///
  /// # Returns
  ///
  /// * スタックに要素がある場合は`Some(&value)`。
  /// * スタックが空の場合は`None`。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert_eq!(stack.peek(), Some(&2));
  /// assert_eq!(stack.len().to_usize(), 2); // 要素は削除されていない
  /// ```
  pub fn peek(&self) -> Option<&T> {
    self.items.last()
  }

  /// スタックからすべての要素を削除します。
  ///
  /// この操作の後、スタックは空になります。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.clear();
  /// assert!(stack.is_empty());
  /// ```
  pub fn clear(&mut self) {
    self.items.clear();
  }
}

/// `StackBuffer`のデフォルト実装。
///
/// `new()`メソッドを呼び出して、容量無制限の空のスタックバッファを作成します。
impl<T> Default for StackBuffer<T> {
  fn default() -> Self {
    Self::new()
  }
}
