use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct RestartStatistics {
  failure_times: Arc<RwLock<Vec<Instant>>>,
}

impl RestartStatistics {
  pub fn new() -> Self {
    Self {
      failure_times: Arc::new(RwLock::new(vec![])),
    }
  }

  pub fn with_values(failure_times: impl IntoIterator<Item = Instant>) -> Self {
    Self {
      failure_times: Arc::new(RwLock::new(failure_times.into_iter().collect())),
    }
  }

  pub async fn failure_count(&self) -> usize {
    let mg = self.failure_times.read().await;
    mg.len()
  }

  pub async fn fail(&mut self) {
    self.push(Instant::now()).await;
  }

  pub async fn push(&mut self, time: Instant) {
    let mut mg = self.failure_times.write().await;
    mg.push(time);
  }

  pub async fn reset(&mut self) {
    let mut mg = self.failure_times.write().await;
    mg.clear();
  }

  pub async fn number_of_failures(&self, within_duration: Duration) -> u32 {
    if within_duration == Duration::ZERO {
      let mg = self.failure_times.read().await;
      return mg.len() as u32;
    }

    let curr_time = Instant::now();
    let mg = self.failure_times.read().await;
    mg.iter()
      .filter(|&&t| curr_time.duration_since(t) < within_duration)
      .count() as u32
  }
}

impl Display for RestartStatistics {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "RestartStatistics")
  }
}

impl PartialEq for RestartStatistics {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.failure_times, &other.failure_times)
  }
}

impl Eq for RestartStatistics {}

impl Hash for RestartStatistics {
  fn hash<H: Hasher>(&self, state: &mut H) {
    Arc::as_ptr(&self.failure_times).hash(state);
  }
}

impl Default for RestartStatistics {
  fn default() -> Self {
    Self::new()
  }
}

static_assertions::assert_impl_all!(RestartStatistics: Send, Sync);
