use std::time::Duration;
use crate::actor::restart_statistics::RestartStatistics;
use crate::actor::supervisor::strategy::SupervisorStrategy;

#[derive(Debug, Clone)]
pub struct ExponentialBackoffStrategy {
    backoff_window: Duration,
    min_backoff: Duration,
    max_backoff: Duration,
    random_factor: f64,
}

impl ExponentialBackoffStrategy {
    pub fn new(
        backoff_window: Duration,
        min_backoff: Duration,
        max_backoff: Duration,
        random_factor: f64,
    ) -> Self {
        Self {
            backoff_window,
            min_backoff,
            max_backoff,
            random_factor,
        }
    }

    async fn handle_failure(&self, rs: &mut RestartStatistics) -> Duration {
        if rs.failure_count == 0 {
            rs.reset();
            Duration::from_secs(0)
        } else {
            rs.failure();
            self.calculate_delay(rs.failure_count)
        }
    }

    fn calculate_delay(&self, failures: u32) -> Duration {
        let backoff = self.min_backoff.as_millis() as f64 * (2_f64.powi(failures as i32 - 1));
        let jittered = backoff * (1.0 + self.random_factor * rand::random::<f64>());
        Duration::from_millis(jittered.min(self.max_backoff.as_millis() as f64) as u64)
    }
}

#[async_trait::async_trait]
impl SupervisorStrategy for ExponentialBackoffStrategy {
    async fn handle_failure(&self, rs: &mut RestartStatistics) -> Duration {
        self.handle_failure(rs).await
    }
}
