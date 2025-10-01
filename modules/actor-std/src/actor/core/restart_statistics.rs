use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nexus_actor_core_rs::actor::core_types::restart::{CoreRestartTracker, FailureClock};
use nexus_utils_std_rs::concurrent::SynchronizedRw;

#[derive(Debug, Clone)]
pub struct RestartStatistics {
  tracker: Arc<SynchronizedRw<CoreRestartTracker>>,
  clock: TokioFailureClock,
}

impl RestartStatistics {
  pub fn new() -> Self {
    Self {
      tracker: Arc::new(SynchronizedRw::new(CoreRestartTracker::new())),
      clock: TokioFailureClock::new(),
    }
  }

  pub fn from_core_tracker(tracker: CoreRestartTracker) -> Self {
    Self {
      tracker: Arc::new(SynchronizedRw::new(tracker)),
      clock: TokioFailureClock::new(),
    }
  }

  pub fn with_values(failure_times: impl IntoIterator<Item = Instant>) -> Self {
    let instants: Vec<Instant> = failure_times.into_iter().collect();
    let anchor = instants.iter().min().copied().unwrap_or_else(Instant::now);
    let clock = TokioFailureClock::with_anchor(anchor);
    let durations = instants
      .into_iter()
      .map(|instant| clock.duration_since_anchor(instant))
      .collect::<Vec<_>>();
    Self {
      tracker: Arc::new(SynchronizedRw::new(CoreRestartTracker::with_values(durations))),
      clock,
    }
  }

  pub async fn failure_count(&self) -> usize {
    self.tracker.read(|t| t.failure_count()).await
  }

  pub async fn fail(&mut self) {
    let clock = self.clock.clone();
    self.tracker.write(|tracker| tracker.record_failure(&clock)).await;
  }

  pub async fn push(&mut self, time: Instant) {
    let duration = self.clock.duration_since_anchor(time);
    self.tracker.write(|tracker| tracker.push(duration)).await;
  }

  pub async fn reset(&mut self) {
    self.tracker.write(|tracker| tracker.reset()).await;
  }

  pub async fn number_of_failures(&self, within_duration: Duration) -> u32 {
    let clock = self.clock.clone();
    let count = self
      .tracker
      .read(|tracker| {
        let now = clock.now();
        tracker.failures_within(now, within_duration)
      })
      .await;
    count.min(u32::MAX as usize) as u32
  }
}

impl RestartStatistics {
  pub async fn snapshot_durations(&self) -> Vec<Duration> {
    self.tracker.read(|tracker| tracker.samples().to_vec()).await
  }

  pub async fn into_core_tracker(self) -> CoreRestartTracker {
    let tracker = self.tracker;
    tracker.read(|guard| (*guard).clone()).await
  }
}

impl Display for RestartStatistics {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "RestartStatistics")
  }
}

impl PartialEq for RestartStatistics {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.tracker, &other.tracker)
  }
}

impl Eq for RestartStatistics {}

impl Hash for RestartStatistics {
  fn hash<H: Hasher>(&self, state: &mut H) {
    Arc::as_ptr(&self.tracker).hash(state);
  }
}

impl Default for RestartStatistics {
  fn default() -> Self {
    Self::new()
  }
}

static_assertions::assert_impl_all!(RestartStatistics: Send, Sync);

#[derive(Debug, Clone)]
struct TokioFailureClock {
  anchor: Arc<Mutex<Instant>>,
}

impl TokioFailureClock {
  fn new() -> Self {
    Self::with_anchor(Instant::now())
  }

  fn with_anchor(anchor: Instant) -> Self {
    Self {
      anchor: Arc::new(Mutex::new(anchor)),
    }
  }

  fn duration_since_anchor(&self, instant: Instant) -> Duration {
    let mut guard = self.anchor.lock().unwrap();
    if instant < *guard {
      *guard = instant;
      Duration::from_secs(0)
    } else {
      instant.duration_since(*guard)
    }
  }
}

impl FailureClock for TokioFailureClock {
  fn now(&self) -> Duration {
    let guard = self.anchor.lock().unwrap();
    Instant::now()
      .checked_duration_since(*guard)
      .unwrap_or_else(|| Duration::from_secs(0))
  }
}

#[cfg(test)]
mod tests {
  use super::RestartStatistics;
  use std::time::{Duration, Instant};

  #[tokio::test]
  async fn tracker_records_failures() {
    let mut stats = RestartStatistics::new();
    stats.fail().await;
    stats.fail().await;
    assert_eq!(stats.failure_count().await, 2);
    assert!(stats.number_of_failures(Duration::from_secs(1)).await >= 1);
  }

  #[tokio::test]
  async fn with_values_initialises_tracker() {
    let now = Instant::now();
    let stats = RestartStatistics::with_values(vec![now - Duration::from_secs(2), now - Duration::from_secs(1)]);
    assert_eq!(stats.failure_count().await, 2);
    assert!(stats.number_of_failures(Duration::from_secs(5)).await >= 2);
  }

  #[tokio::test]
  async fn reset_clears_state() {
    let mut stats = RestartStatistics::new();
    stats.fail().await;
    stats.reset().await;
    assert_eq!(stats.failure_count().await, 0);
  }
}
