use super::strategy::{SupervisorDirective, SupervisorStrategy};
use crate::actor::actor::Actor;
use crate::actor::context::Context;
use crate::actor::pid::Pid;
use crate::actor::restart_statistics::RestartStatistics;
use async_trait::async_trait;
use std::error::Error;
use std::time::Duration;

#[derive(Debug)]
pub struct AllForOneStrategy {
    max_retries: i32,
    within_time: Duration,
    decider: Box<dyn Fn(Box<dyn Error>) -> SupervisorDirective + Send + Sync>,
}

impl AllForOneStrategy {
    pub fn new<F>(max_retries: i32, within_time: Duration, decider: F) -> Self
    where
        F: Fn(Box<dyn Error>) -> SupervisorDirective + Send + Sync + 'static,
    {
        Self {
            max_retries,
            within_time,
            decider: Box::new(decider),
        }
    }
}

#[async_trait]
impl SupervisorStrategy for AllForOneStrategy {
    async fn handle_failure(
        &self,
        ctx: &Context,
        pid: &Pid,
        restart_statistics: &RestartStatistics,
        reason: Box<dyn Error>,
        message: Option<Box<dyn Actor>>,
    ) {
        let directive = (self.decider)(reason);
        let children = ctx.children().await;
        
        match directive {
            SupervisorDirective::Resume => {
                ctx.resume_children(&children).await;
            }
            SupervisorDirective::Restart => {
                if restart_statistics.should_restart(self.max_retries, self.within_time) {
                    ctx.restart_children(&children).await;
                } else {
                    ctx.stop_children(&children).await;
                }
            }
            SupervisorDirective::Stop => {
                ctx.stop_children(&children).await;
            }
            SupervisorDirective::Escalate => {
                ctx.escalate_failure(reason, message).await;
            }
        }
    }
}
