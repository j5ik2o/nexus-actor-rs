use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct RestartStatistics {
    pub failure_count: u32,
    pub last_failure_time: SystemTime,
}

impl RestartStatistics {
    pub fn new() -> Self {
        Self {
            failure_count: 0,
            last_failure_time: SystemTime::now(),
        }
    }

    pub fn failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = SystemTime::now();
    }

    pub fn reset(&mut self) {
        self.failure_count = 0;
        self.last_failure_time = SystemTime::now();
    }

    pub fn should_restart(&self, max_retries: u32, within_time: Duration) -> bool {
        if self.failure_count > max_retries {
            return false;
        }

        if let Ok(elapsed) = self.last_failure_time.elapsed() {
            elapsed < within_time
        } else {
            false
        }
    }
}

impl Default for RestartStatistics {
    fn default() -> Self {
        Self::new()
    }
}
