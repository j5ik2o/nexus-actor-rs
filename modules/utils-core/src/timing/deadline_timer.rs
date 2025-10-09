use core::sync::atomic::{AtomicUsize, Ordering};
use core::{
  fmt,
  task::{Context, Poll},
  time::Duration,
};

/// DeadlineTimer に登録されたアイテムを識別するためのキー。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct DeadlineTimerKey(u64);

impl DeadlineTimerKey {
  /// 無効なキーを返す。
  #[inline]
  pub const fn invalid() -> Self {
    Self(0)
  }

  /// キーが有効か判定する。
  #[inline]
  pub const fn is_valid(self) -> bool {
    self.0 != 0
  }

  /// 内部表現へアクセスする。
  #[inline]
  pub const fn into_raw(self) -> u64 {
    self.0
  }

  /// 内部表現からキーを生成する。
  #[inline]
  pub const fn from_raw(raw: u64) -> Self {
    Self(raw)
  }
}

/// DeadlineTimer の期限を表す新しい型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerDeadline(Duration);

impl TimerDeadline {
  /// 指定した継続時間から期限を作成する。
  #[inline]
  pub const fn from_duration(duration: Duration) -> Self {
    Self(duration)
  }

  /// 格納されている継続時間を取得する。
  #[inline]
  pub const fn as_duration(self) -> Duration {
    self.0
  }
}

impl From<Duration> for TimerDeadline {
  #[inline]
  fn from(value: Duration) -> Self {
    Self::from_duration(value)
  }
}

impl From<TimerDeadline> for Duration {
  #[inline]
  fn from(value: TimerDeadline) -> Self {
    value.as_duration()
  }
}

/// DeadlineTimer 操作で発生し得るエラー。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadlineTimerError {
  /// 指定したキーに対応する項目が存在しない。
  KeyNotFound,
  /// DeadlineTimer が既に停止しているなどの理由で操作できない。
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
#[derive(Debug)]
pub struct DeadlineTimerExpired<Item> {
  pub key: DeadlineTimerKey,
  pub item: Item,
}

/// DeadlineTimer の振る舞いを抽象化したトレイト。
///
/// `no_std` 環境でも利用可能な最小 API を提供する。
pub trait DeadlineTimer {
  /// キューが保持する要素の型。
  type Item;
  /// 操作時に発生し得るエラー型。
  type Error;

  /// 新しい要素を期限付きで挿入する。
  fn insert(&mut self, item: Self::Item, deadline: TimerDeadline) -> Result<DeadlineTimerKey, Self::Error>;

  /// 指定キーの要素を期限を更新して再登録する。
  fn reset(&mut self, key: DeadlineTimerKey, deadline: TimerDeadline) -> Result<(), Self::Error>;

  /// 指定キーの要素をキャンセルし、要素を返す。
  fn cancel(&mut self, key: DeadlineTimerKey) -> Result<Option<Self::Item>, Self::Error>;

  /// 最も近い期限の要素をポーリングする。
  fn poll_expired(&mut self, cx: &mut Context<'_>) -> Poll<Result<DeadlineTimerExpired<Self::Item>, Self::Error>>;
}

/// DeadlineTimerKey を生成するためのアロケータ。
#[derive(Debug)]
pub struct DeadlineTimerKeyAllocator {
  counter: AtomicUsize,
}

impl DeadlineTimerKeyAllocator {
  /// 新しいアロケータを作成する。
  #[inline]
  pub const fn new() -> Self {
    Self {
      counter: AtomicUsize::new(1),
    }
  }

  /// 新しい一意なキーを払い出す。
  #[inline]
  pub fn allocate(&self) -> DeadlineTimerKey {
    let next = self.counter.fetch_add(1, Ordering::Relaxed) as u64;
    let raw = if next == 0 { 1 } else { next };
    DeadlineTimerKey::from_raw(raw)
  }

  /// 次に払い出されるキーを確認する（テスト用途）。
  #[inline]
  pub fn peek(&self) -> DeadlineTimerKey {
    DeadlineTimerKey::from_raw(self.counter.load(Ordering::Relaxed) as u64)
  }
}

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
