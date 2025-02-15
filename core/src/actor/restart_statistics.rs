use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct RestartStatistics {
    failure_count: u32,
    last_failure: SystemTime,
}

impl RestartStatistics {
    pub fn new() -> Self {
        Self {
            failure_count: 0,
            last_failure: SystemTime::now(),
        }
    }

    pub fn failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = SystemTime::now();
    }

    pub fn reset(&mut self) {
        self.failure_count = 0;
        self.last_failure = SystemTime::now();
    }

    pub fn number_of_failures(&self, within: Duration) -> u32 {
        if let Ok(elapsed) = SystemTime::now().duration_since(self.last_failure) {
            if elapsed <= within {
                self.failure_count
            } else {
                0
            }
        } else {
            0
        }
    }
}

impl Default for RestartStatistics {
    fn default() -> Self {
        Self::new()
    }
}
