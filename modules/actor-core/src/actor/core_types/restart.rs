#![cfg(feature = "alloc")]

use alloc::vec::Vec;
use core::any::Any;
use core::time::Duration;

/// 単調増加する時刻を提供するクロック抽象。
///
/// `now()` は任意の基準からの経過時間を `Duration` で返す必要がある。
pub trait FailureClock: Send + Sync + 'static {
  /// 現在時刻を基準からの経過時間として返す。
  fn now(&self) -> Duration;

  /// Downcast 用に Any 参照を返す。
  fn as_any(&self) -> &dyn Any;
}

/// no_std + alloc 環境で利用可能な単純な再起動統計。
///
/// 時刻を `Duration` として保持し、呼び出し側が提供するクロックに依存する。
#[derive(Clone, Default)]
pub struct CoreRestartTracker {
  failure_times: Vec<Duration>,
  history_limit: Option<usize>,
}

impl CoreRestartTracker {
  /// 空のトラッカーを作成する。
  #[must_use]
  pub const fn new() -> Self {
    Self {
      failure_times: Vec::new(),
      history_limit: None,
    }
  }

  /// 最大保持数を指定してトラッカーを作成する。
  #[must_use]
  pub fn with_history_limit(limit: usize) -> Self {
    Self {
      failure_times: Vec::with_capacity(limit),
      history_limit: Some(limit),
    }
  }

  /// 既存の時刻列から初期化する。
  #[must_use]
  pub fn with_values<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Duration>, {
    let mut tracker = Self::new();
    tracker.failure_times.extend(values);
    tracker
  }

  /// 失敗時刻を追加し、必要に応じて履歴を剪定する。
  pub fn push(&mut self, instant: Duration) {
    if let Some(limit) = self.history_limit {
      if self.failure_times.len() == limit {
        // 古い要素を 1 件落とす（FIFO 振る舞い）。
        self.failure_times.remove(0);
      }
    }
    self.failure_times.push(instant);
  }

  /// クロックから現在時刻を取得して記録する。
  pub fn record_failure<C: FailureClock>(&mut self, clock: &C) {
    let now = clock.now();
    self.push(now);
  }

  /// 現在登録されている失敗回数を返す。
  #[must_use]
  pub fn failure_count(&self) -> usize {
    self.failure_times.len()
  }

  /// 登録されている履歴を全て削除する。
  pub fn reset(&mut self) {
    self.failure_times.clear();
  }

  /// `window` 以内に発生した失敗回数を返す。`window == 0` なら総数を返す。
  #[must_use]
  pub fn failures_within(&self, now: Duration, window: Duration) -> usize {
    if window.is_zero() {
      return self.failure_times.len();
    }
    self
      .failure_times
      .iter()
      .rev()
      .take_while(|&&instant| {
        if now < instant {
          return false;
        }
        now.saturating_sub(instant) < window
      })
      .count()
  }

  /// `window` 以前の履歴を削除する。
  pub fn prune(&mut self, now: Duration, window: Duration) {
    if window.is_zero() {
      return;
    }
    let cutoff = now.saturating_sub(window);
    let retain_from = self
      .failure_times
      .iter()
      .position(|instant| *instant > cutoff)
      .unwrap_or(self.failure_times.len());
    if retain_from > 0 {
      self.failure_times.drain(0..retain_from);
    }
  }

  /// 保持している時刻のスライスを取得する（主にテスト用）。
  #[must_use]
  pub fn samples(&self) -> &[Duration] {
    &self.failure_times
  }
}

#[cfg(test)]
mod tests {
  use super::{CoreRestartTracker, FailureClock};
  use core::any::Any;
  use core::sync::atomic::{AtomicU64, Ordering};
  use core::time::Duration;

  #[derive(Default)]
  struct MockClock {
    nanos: AtomicU64,
  }

  impl MockClock {
    fn advance(&self, delta: Duration) {
      let delta_nanos = delta.as_nanos() as u64;
      self.nanos.fetch_add(delta_nanos, Ordering::SeqCst);
    }
  }

  impl FailureClock for MockClock {
    fn now(&self) -> Duration {
      Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
    }

    fn as_any(&self) -> &dyn Any {
      self
    }
  }

  #[test]
  fn record_and_measure_failures() {
    let clock = MockClock::default();
    let mut tracker = CoreRestartTracker::new();

    tracker.record_failure(&clock);
    clock.advance(Duration::from_secs(1));
    tracker.record_failure(&clock);
    clock.advance(Duration::from_secs(1));
    tracker.record_failure(&clock);

    assert_eq!(tracker.failure_count(), 3);

    let now = clock.now();
    assert_eq!(tracker.failures_within(now, Duration::from_secs(1)), 1);
    assert_eq!(tracker.failures_within(now, Duration::from_secs(2)), 2);
    assert_eq!(tracker.failures_within(now, Duration::from_secs(5)), 3);
  }

  #[test]
  fn prune_old_samples() {
    let mut tracker = CoreRestartTracker::new();

    tracker.push(Duration::from_secs(1));
    tracker.push(Duration::from_secs(2));
    tracker.push(Duration::from_secs(10));

    tracker.prune(Duration::from_secs(10), Duration::from_secs(5));
    assert_eq!(tracker.samples(), &[Duration::from_secs(10)]);
  }

  #[test]
  fn history_limit_drops_oldest() {
    let mut tracker = CoreRestartTracker::with_history_limit(2);
    tracker.push(Duration::from_secs(1));
    tracker.push(Duration::from_secs(2));
    tracker.push(Duration::from_secs(3));

    assert_eq!(tracker.samples(), &[Duration::from_secs(2), Duration::from_secs(3)]);
  }
}

impl core::fmt::Debug for CoreRestartTracker {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("CoreRestartTracker")
      .field("samples", &self.failure_times)
      .field("history_limit", &self.history_limit)
      .finish()
  }
}
