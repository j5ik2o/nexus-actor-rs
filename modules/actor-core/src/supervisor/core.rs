#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::future::Future;
use core::pin::Pin;

use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::pid::CorePid;
use crate::actor::core_types::restart::CoreRestartTracker;
use crate::error::ErrorReasonCore;

pub type CoreSupervisorFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type CoreSupervisorStrategyFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSupervisorDirective {
  Resume,
  Restart,
  Stop,
  Escalate,
}

pub trait CoreSupervisorContext: Any + Send + Sync {
  fn now(&self) -> u64;
}

pub trait CoreSupervisorStrategy: Any + Send + Sync {
  fn handle_child_failure<'a>(
    &'a self,
    ctx: &'a dyn CoreSupervisorContext,
    supervisor: &'a dyn CoreSupervisor,
    child: CorePid,
    tracker: &'a mut CoreRestartTracker,
    reason: ErrorReasonCore,
    message: MessageHandle,
  ) -> CoreSupervisorStrategyFuture<'a>;
}

pub trait CoreSupervisor: Any + Send + Sync {
  fn children<'a>(&'a self) -> CoreSupervisorFuture<'a, Vec<CorePid>>;
  fn apply_directive<'a>(
    &'a self,
    directive: CoreSupervisorDirective,
    targets: &'a [CorePid],
    reason: ErrorReasonCore,
    message: MessageHandle,
  ) -> CoreSupervisorFuture<'a, ()>;
  fn escalate<'a>(&'a self, reason: ErrorReasonCore, message: MessageHandle) -> CoreSupervisorFuture<'a, ()>;
}
