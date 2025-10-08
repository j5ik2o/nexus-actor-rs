use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nexus_actor_core_rs::actor::core_types::restart::{CoreRestartTracker, FailureClock};
use nexus_actor_core_rs::runtime::CoreRuntime;
use nexus_utils_std_rs::concurrent::SynchronizedRw;
use nexus_utils_std_rs::runtime::sync::InstantFailureClock;

#[derive(Debug, Clone)]
pub struct RestartStatistics {
  tracker: Arc<SynchronizedRw<CoreRestartTracker>>,
  clock: InstantFailureClock,
}

impl RestartStatistics {
  pub fn new() -> Self {
    Self {
      tracker: Arc::new(SynchronizedRw::new(CoreRestartTracker::new())),
      clock: InstantFailureClock::new(),
    }
  }

  pub fn with_runtime(runtime: &CoreRuntime) -> Self {
    let mut stats = Self::new();
    if let Some(clock) = runtime.failure_clock() {
      if let Some(inst_clock) = clock.as_any().downcast_ref::<InstantFailureClock>() {
        stats.clock = inst_clock.clone();
      }
    }
    stats
  }

  pub fn from_core_tracker(tracker: CoreRestartTracker) -> Self {
    Self {
      tracker: Arc::new(SynchronizedRw::new(tracker)),
      clock: InstantFailureClock::new(),
    }
  }

  pub fn from_core_tracker_with_anchor(tracker: CoreRestartTracker, anchor: Duration) -> Self {
    let now = Instant::now();
    let anchor_instant = now.checked_sub(anchor).unwrap_or(now);
    Self {
      tracker: Arc::new(SynchronizedRw::new(tracker)),
      clock: InstantFailureClock::with_anchor(anchor_instant),
    }
  }

  pub fn from_core_tracker_with_clock(tracker: CoreRestartTracker, clock: InstantFailureClock) -> Self {
    Self {
      tracker: Arc::new(SynchronizedRw::new(tracker)),
      clock,
    }
  }

  pub fn with_values(failure_times: impl IntoIterator<Item = Instant>) -> Self {
    let instants: Vec<Instant> = failure_times.into_iter().collect();
    let anchor = instants.iter().min().copied().unwrap_or_else(Instant::now);
    let clock = InstantFailureClock::with_anchor(anchor);
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

  pub async fn snapshot_durations(&self) -> Vec<Duration> {
    self.tracker.read(|tracker| tracker.samples().to_vec()).await
  }

  pub async fn to_core_tracker(&self) -> CoreRestartTracker {
    self.tracker.read(|tracker| (*tracker).clone()).await
  }

  pub async fn overwrite_with(&self, tracker: CoreRestartTracker) {
    self
      .tracker
      .write(|inner| {
        **inner = tracker.clone();
      })
      .await;
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

#[cfg(test)]
mod tests {
  use super::RestartStatistics;
  use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
  use nexus_utils_std_rs::runtime::sync::{tokio_core_runtime, InstantFailureClock};
  use std::time::{Duration, Instant};
  use tokio::time::sleep;

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

  #[tokio::test]
  async fn with_runtime_uses_injected_failure_clock() {
    let factory = tokio_core_runtime();
    let mut stats = RestartStatistics::with_runtime(&runtime);

    stats.fail().await;
    sleep(Duration::from_millis(25)).await;
    stats.fail().await;

    let samples = stats.snapshot_durations().await;
    assert_eq!(samples.len(), 2);
    assert!(samples[1] > samples[0]);
    assert!(samples[1] >= Duration::from_millis(25));
  }

  #[tokio::test]
  async fn from_core_tracker_with_clock_preserves_samples() {
    let durations = vec![Duration::from_millis(5), Duration::from_millis(20)];
    let tracker = CoreRestartTracker::with_values(durations.clone());
    let clock = InstantFailureClock::with_anchor(Instant::now());

    let stats = RestartStatistics::from_core_tracker_with_clock(tracker, clock);

    let snapshot = stats.snapshot_durations().await;
    assert_eq!(snapshot, durations);
  }
}
