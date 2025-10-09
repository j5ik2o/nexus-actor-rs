use core::fmt::{self, Debug, Formatter};

#[cfg(feature = "alloc")]
use alloc::sync::Arc;

#[cfg(not(feature = "alloc"))]
use core::cell::Cell;
#[cfg(feature = "alloc")]
use core::sync::atomic::{AtomicBool, Ordering};

/// スレッドセーフなブールフラグを提供する構造体
///
/// `Flag`は、マルチスレッド環境で安全に使用できるブールフラグを提供します。
///
/// # 実装の詳細
///
/// - `alloc`フィーチャが有効な場合: `Arc<AtomicBool>`を使用してスレッドセーフな実装を提供
/// - `alloc`フィーチャが無効な場合: `Cell<bool>`を使用してシングルスレッド環境向けの軽量な実装を提供
///
/// # 例
///
/// ```
/// use nexus_utils_core_rs::Flag;
///
/// let flag = Flag::new(false);
/// assert!(!flag.get());
///
/// flag.set(true);
/// assert!(flag.get());
///
/// flag.clear();
/// assert!(!flag.get());
/// ```
#[derive(Clone)]
pub struct Flag {
  #[cfg(feature = "alloc")]
  inner: Arc<AtomicBool>,
  #[cfg(not(feature = "alloc"))]
  inner: Cell<bool>,
}

impl Flag {
  /// 指定された初期値で新しい`Flag`を作成します
  ///
  /// # 引数
  ///
  /// * `value` - フラグの初期値
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(true);
  /// assert!(flag.get());
  /// ```
  pub fn new(value: bool) -> Self {
    #[cfg(feature = "alloc")]
    {
      Self {
        inner: Arc::new(AtomicBool::new(value)),
      }
    }

    #[cfg(not(feature = "alloc"))]
    {
      Self {
        inner: Cell::new(value),
      }
    }
  }

  /// フラグの値を設定します
  ///
  /// # 引数
  ///
  /// * `value` - 設定する新しい値
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(false);
  /// flag.set(true);
  /// assert!(flag.get());
  /// ```
  pub fn set(&self, value: bool) {
    #[cfg(feature = "alloc")]
    {
      self.inner.store(value, Ordering::SeqCst);
    }

    #[cfg(not(feature = "alloc"))]
    {
      self.inner.set(value);
    }
  }

  /// フラグの現在の値を取得します
  ///
  /// # 戻り値
  ///
  /// フラグの現在の値
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(true);
  /// assert!(flag.get());
  /// ```
  pub fn get(&self) -> bool {
    #[cfg(feature = "alloc")]
    {
      self.inner.load(Ordering::SeqCst)
    }

    #[cfg(not(feature = "alloc"))]
    {
      return self.inner.get();
    }
  }

  /// フラグをクリアします（`false`に設定します）
  ///
  /// このメソッドは`set(false)`と同等です。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_core_rs::Flag;
  ///
  /// let flag = Flag::new(true);
  /// flag.clear();
  /// assert!(!flag.get());
  /// ```
  pub fn clear(&self) {
    self.set(false);
  }
}

impl Default for Flag {
  fn default() -> Self {
    Self::new(false)
  }
}

impl Debug for Flag {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("Flag").field("value", &self.get()).finish()
  }
}
