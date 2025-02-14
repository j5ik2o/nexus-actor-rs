use super::strategy::{SupervisorDirective, SupervisorStrategy};
use crate::actor::actor::Actor;
use crate::actor::context::Context;
use crate::actor::pid::Pid;
use crate::actor::restart_statistics::RestartStatistics;
use async_trait::async_trait;
use std::error::Error;
use std::time::Duration;

#[derive(Debug)]
pub struct ExponentialBackoffStrategy {
    min_backoff: Duration,
    max_backoff: Duration,
    random_factor: f64,
}

impl ExponentialBackoffStrategy {
    pub fn new(min_backoff: Duration, max_backoff: Duration, random_factor: f64) -> Self {
        Self {
            min_backoff,
            max_backoff,
            random_factor,
        }
    }

    fn calculate_backoff(&self, attempt: i32) -> Duration {
        let backoff = self.min_backoff.as_millis() as f64 * (2_f64.powi(attempt - 1));
        let max = self.max_backoff.as_millis() as f64;
        let random_factor = 1.0 + (rand::random::<f64>() * self.random_factor);
        
        Duration::from_millis((backoff.min(max) * random_factor) as u64)
    }
}

#[async_trait]
impl SupervisorStrategy for ExponentialBackoffStrategy {
    async fn handle_failure(
        &self,
        ctx: &Context,
        pid: &Pid,
        restart_statistics: &RestartStatistics,
        reason: Box<dyn Error>,
        message: Option<Box<dyn Actor>>,
    ) {
        let backoff = self.calculate_backoff(restart_statistics.failure_count());
        tokio::time::sleep(backoff).await;
        
        if backoff < self.max_backoff {
            ctx.restart_children(&[pid.clone()]).await;
        } else {
            ctx.stop_children(&[pid.clone()]).await;
        }
    }
}
