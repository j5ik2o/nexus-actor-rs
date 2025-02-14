use crate::actor::actor::Actor;
use crate::actor::context::Context;
use crate::actor::pid::Pid;
use crate::actor::restart_statistics::RestartStatistics;
use async_trait::async_trait;
use std::error::Error;
use std::fmt::Debug;

#[async_trait]
pub trait SupervisorStrategy: Debug + Send + Sync {
    async fn handle_failure(
        &self,
        ctx: &Context,
        pid: &Pid,
        restart_statistics: &RestartStatistics,
        reason: Box<dyn Error>,
        message: Option<Box<dyn Actor>>,
    );
}

#[derive(Debug, Clone, Copy)]
pub enum SupervisorDirective {
    Resume,
    Restart,
    Stop,
    Escalate,
}
