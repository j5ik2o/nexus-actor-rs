use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RestartStatistics {
  failure_times: Vec<Instant>,
}

impl RestartStatistics {
  pub fn new() -> Self {
    RestartStatistics {
      failure_times: Vec::new(),
    }
  }

  pub fn failure_count(&self) -> usize {
    self.failure_times.len()
  }

  pub fn fail(&mut self) {
    self.failure_times.push(Instant::now());
  }

  pub fn reset(&mut self) {
    self.failure_times.clear();
  }

  pub fn number_of_failures(&self, within_duration: Duration) -> u32 {
    if within_duration == Duration::ZERO {
      return self.failure_times.len() as u32;
    }

    let curr_time = Instant::now();
    self
      .failure_times
      .iter()
      .filter(|&&t| curr_time.duration_since(t) < within_duration)
      .count() as u32
  }
}

impl Default for RestartStatistics {
  fn default() -> Self {
    Self::new()
  }
}
