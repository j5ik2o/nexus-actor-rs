use std::time::Duration;
use async_trait::async_trait;
use crate::actor::{
    RestartStatistics,
    supervisor::strategy::SupervisorStrategy,
};

#[derive(Debug, Clone)]
pub struct OneForOneStrategy {
    max_nr_of_retries: u32,
    within_duration: Duration,
}

impl OneForOneStrategy {
    pub fn new(max_nr_of_retries: u32, within_duration: Duration) -> Self {
        Self {
            max_nr_of_retries,
            within_duration,
        }
    }
}

#[async_trait]
impl SupervisorStrategy for OneForOneStrategy {
    async fn handle_failure(&self, rs: &mut RestartStatistics) -> Duration {
        rs.failure();
        if rs.number_of_failures(self.within_duration) > self.max_nr_of_retries {
            rs.reset();
            Duration::from_secs(0)
        } else {
            Duration::from_millis(100)
        }
    }
}
