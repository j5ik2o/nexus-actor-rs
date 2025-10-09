use core::sync::atomic::{AtomicUsize, Ordering};
use core::{
  fmt,
  task::{Context, Poll},
  time::Duration,
};

/// DeadlineTimer に登録されたアイテムを識別するためのキー。
///
/// このキーは、タイマーに登録された各アイテムを一意に識別するために使用されます。
/// 内部的には64ビット整数として表現され、0は無効なキーとして予約されています。
///
/// # 例
///
/// ```
/// use nexus_actor_utils_core::timing::DeadlineTimerKey;
///
/// let key = DeadlineTimerKey::from_raw(42);
/// assert!(key.is_valid());
/// assert_eq!(key.into_raw(), 42);
///
/// let invalid = DeadlineTimerKey::invalid();
/// assert!(!invalid.is_valid());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct DeadlineTimerKey(u64);

impl DeadlineTimerKey {
  /// 無効なキーを返す。
  ///
  /// 無効なキーは内部的に0として表現され、タイマーに登録されていない
  /// 状態を示すために使用されます。
  #[inline]
  pub const fn invalid() -> Self {
    Self(0)
  }

  /// キーが有効か判定する。
  ///
  /// # Returns
  ///
  /// キーが有効（0以外）であれば`true`、無効（0）であれば`false`を返します。
  #[inline]
  pub const fn is_valid(self) -> bool {
    self.0 != 0
  }

  /// 内部表現へアクセスする。
  ///
  /// キーの内部的な64ビット整数表現を取得します。
  ///
  /// # Returns
  ///
  /// キーの内部表現である`u64`値を返します。
  #[inline]
  pub const fn into_raw(self) -> u64 {
    self.0
  }

  /// 内部表現からキーを生成する。
  ///
  /// # Arguments
  ///
  /// * `raw` - キーの内部表現となる64ビット整数
  ///
  /// # Returns
  ///
  /// 指定された整数値から生成されたキーを返します。
  #[inline]
  pub const fn from_raw(raw: u64) -> Self {
    Self(raw)
  }
}

/// DeadlineTimer の期限を表す新しい型。
///
/// タイマーの期限を型安全に扱うためのラッパー型です。
/// 内部的には`Duration`として表現され、タイマーに登録されたアイテムが
/// 期限切れになるまでの時間を表します。
///
/// # 例
///
/// ```
/// use nexus_actor_utils_core::timing::TimerDeadline;
/// use core::time::Duration;
///
/// let duration = Duration::from_secs(5);
/// let deadline = TimerDeadline::from_duration(duration);
/// assert_eq!(deadline.as_duration(), duration);
///
/// // From/Into traitsによる変換も可能
/// let deadline2: TimerDeadline = Duration::from_millis(100).into();
/// let duration2: Duration = deadline2.into();
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerDeadline(Duration);

impl TimerDeadline {
  /// 指定した継続時間から期限を作成する。
  ///
  /// # Arguments
  ///
  /// * `duration` - 期限までの継続時間
  ///
  /// # Returns
  ///
  /// 指定された継続時間をラップした`TimerDeadline`を返します。
  #[inline]
  pub const fn from_duration(duration: Duration) -> Self {
    Self(duration)
  }

  /// 格納されている継続時間を取得する。
  ///
  /// # Returns
  ///
  /// ラップされている`Duration`値を返します。
  #[inline]
  pub const fn as_duration(self) -> Duration {
    self.0
  }
}

/// `Duration`から`TimerDeadline`への変換を提供します。
impl From<Duration> for TimerDeadline {
  #[inline]
  fn from(value: Duration) -> Self {
    Self::from_duration(value)
  }
}

/// `TimerDeadline`から`Duration`への変換を提供します。
impl From<TimerDeadline> for Duration {
  #[inline]
  fn from(value: TimerDeadline) -> Self {
    value.as_duration()
  }
}

/// DeadlineTimer 操作で発生し得るエラー。
///
/// タイマー操作中に発生する可能性のあるエラーを表します。
///
/// # バリアント
///
/// * `KeyNotFound` - 指定されたキーに対応するアイテムが存在しない場合
/// * `Closed` - タイマーが既に停止しているなどの理由で操作できない場合
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadlineTimerError {
  /// 指定したキーに対応する項目が存在しない。
  ///
  /// `reset`や`cancel`操作時に、指定されたキーがタイマーに登録されていない
  /// 場合に返されます。
  KeyNotFound,
  /// DeadlineTimer が既に停止しているなどの理由で操作できない。
  ///
  /// タイマーがクローズされた後に操作を試みた場合に返されます。
  Closed,
}

impl fmt::Display for DeadlineTimerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      DeadlineTimerError::KeyNotFound => write!(f, "key not found"),
      DeadlineTimerError::Closed => write!(f, "deadline timer is closed"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for DeadlineTimerError {}

/// DeadlineTimer の期限切れイベント。
///
/// タイマーに登録されたアイテムが期限切れになった際に返されるイベントです。
/// 期限切れになったアイテムと、そのアイテムを識別するためのキーを含みます。
///
/// # フィールド
///
/// * `key` - 期限切れになったアイテムのキー
/// * `item` - 期限切れになったアイテム本体
#[derive(Debug)]
pub struct DeadlineTimerExpired<Item> {
  /// 期限切れになったアイテムのキー
  pub key: DeadlineTimerKey,
  /// 期限切れになったアイテム本体
  pub item: Item,
}

/// DeadlineTimer の振る舞いを抽象化したトレイト。
///
/// `ReceiveTimeout` をはじめ、期限付きで要素を管理したい仕組みが
/// ランタイムごとに異なる実装を差し替えられるよう、挿入・再設定・キャンセル・期限待ちの操作だけを定義しています。
/// `no_std` でも扱えるよう、標準ライブラリ依存のない API に絞っています。
///
/// # 関連型
///
/// * `Item` - タイマーが管理するアイテムの型
/// * `Error` - タイマー操作で発生し得るエラーの型
///
/// # 例
///
/// ```ignore
/// use nexus_actor_utils_core::timing::{DeadlineTimer, TimerDeadline};
/// use core::time::Duration;
///
/// fn schedule_timeout<T: DeadlineTimer>(timer: &mut T, item: T::Item) {
///     let deadline = TimerDeadline::from_duration(Duration::from_secs(5));
///     let key = timer.insert(item, deadline).expect("Failed to insert");
///     // 必要に応じてキーを保存して後でキャンセルや再設定を行う
/// }
/// ```
pub trait DeadlineTimer {
  /// タイマーが保持する要素の型。
  type Item;
  /// 操作時に発生し得るエラー型。
  type Error;

  /// 新しい要素を期限付きで挿入する。
  ///
  /// # Arguments
  ///
  /// * `item` - タイマーに登録するアイテム
  /// * `deadline` - アイテムが期限切れになるまでの期限
  ///
  /// # Returns
  ///
  /// 成功時は登録されたアイテムを識別するための`DeadlineTimerKey`を返します。
  /// 失敗時はエラーを返します。
  fn insert(&mut self, item: Self::Item, deadline: TimerDeadline) -> Result<DeadlineTimerKey, Self::Error>;

  /// 指定キーの要素の期限を更新して再登録する。
  ///
  /// # Arguments
  ///
  /// * `key` - 更新対象のアイテムのキー
  /// * `deadline` - 新しい期限
  ///
  /// # Returns
  ///
  /// 成功時は`Ok(())`、失敗時はエラーを返します。
  /// キーが存在しない場合は`KeyNotFound`エラーとなります。
  fn reset(&mut self, key: DeadlineTimerKey, deadline: TimerDeadline) -> Result<(), Self::Error>;

  /// 指定キーの要素をキャンセルし、要素を返す。
  ///
  /// # Arguments
  ///
  /// * `key` - キャンセルするアイテムのキー
  ///
  /// # Returns
  ///
  /// 成功時はキャンセルされたアイテムを`Some`で返します。
  /// キーが存在しない場合は`None`を返します。
  /// タイマーがクローズされている場合はエラーを返します。
  fn cancel(&mut self, key: DeadlineTimerKey) -> Result<Option<Self::Item>, Self::Error>;

  /// 最も近い期限の要素をポーリングする。
  ///
  /// # Arguments
  ///
  /// * `cx` - 非同期タスクのコンテキスト
  ///
  /// # Returns
  ///
  /// * `Poll::Ready(Ok(expired))` - 期限切れのアイテムがある場合
  /// * `Poll::Pending` - まだ期限切れのアイテムがない場合
  /// * `Poll::Ready(Err(e))` - エラーが発生した場合
  fn poll_expired(&mut self, cx: &mut Context<'_>) -> Poll<Result<DeadlineTimerExpired<Self::Item>, Self::Error>>;
}

/// DeadlineTimerKey を生成するためのアロケータ。
///
/// スレッドセーフに一意なキーを生成するためのアロケータです。
/// 内部的にはアトミックカウンタを使用して、重複のないキーを払い出します。
///
/// # 例
///
/// ```
/// use nexus_actor_utils_core::timing::DeadlineTimerKeyAllocator;
///
/// let allocator = DeadlineTimerKeyAllocator::new();
/// let key1 = allocator.allocate();
/// let key2 = allocator.allocate();
/// assert_ne!(key1, key2);
/// assert!(key1.is_valid());
/// assert!(key2.is_valid());
/// ```
#[derive(Debug)]
pub struct DeadlineTimerKeyAllocator {
  counter: AtomicUsize,
}

impl DeadlineTimerKeyAllocator {
  /// 新しいアロケータを作成する。
  ///
  /// カウンタは1から開始され、0は無効なキーとして予約されています。
  ///
  /// # Returns
  ///
  /// 新しく作成された`DeadlineTimerKeyAllocator`を返します。
  #[inline]
  pub const fn new() -> Self {
    Self {
      counter: AtomicUsize::new(1),
    }
  }

  /// 新しい一意なキーを払い出す。
  ///
  /// この操作はスレッドセーフで、複数のスレッドから同時に呼び出しても
  /// 常に一意なキーが返されます。
  ///
  /// # Returns
  ///
  /// 新しく生成された一意な`DeadlineTimerKey`を返します。
  ///
  /// # Panics
  ///
  /// カウンタがオーバーフローした場合でも、1から再開するため安全です。
  #[inline]
  pub fn allocate(&self) -> DeadlineTimerKey {
    let next = self.counter.fetch_add(1, Ordering::Relaxed) as u64;
    let raw = if next == 0 { 1 } else { next };
    DeadlineTimerKey::from_raw(raw)
  }

  /// 次に払い出されるキーを確認する（テスト用途）。
  ///
  /// この操作は実際にキーを払い出さず、次に`allocate`が返すであろう
  /// キーを確認するだけです。主にテストやデバッグ用途で使用されます。
  ///
  /// # Returns
  ///
  /// 次に`allocate`が返すと予想される`DeadlineTimerKey`を返します。
  ///
  /// # 注意
  ///
  /// この操作と実際の`allocate`の間に他のスレッドが介入する可能性があるため、
  /// 返されたキーが実際に次に払い出されるとは限りません。
  #[inline]
  pub fn peek(&self) -> DeadlineTimerKey {
    DeadlineTimerKey::from_raw(self.counter.load(Ordering::Relaxed) as u64)
  }
}

/// `Default` トレイトの実装。
///
/// `new()` と同じ動作で新しいアロケータを作成します。
impl Default for DeadlineTimerKeyAllocator {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use std::collections::HashSet;

  #[test]
  fn allocate_provides_unique_keys() {
    let allocator = DeadlineTimerKeyAllocator::new();
    let mut keys = HashSet::new();

    for _ in 0..1024 {
      let key = allocator.allocate();
      assert!(key.is_valid());
      assert!(keys.insert(key.into_raw()));
    }
  }

  #[test]
  fn deadline_roundtrip() {
    let duration = Duration::from_millis(150);
    let deadline = TimerDeadline::from(duration);
    assert_eq!(deadline.as_duration(), duration);
    let back: Duration = deadline.into();
    assert_eq!(back, duration);
  }
}
