#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use core::future::Future;
use core::pin::Pin;

use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::pid::CorePid;
use crate::actor::core_types::restart::CoreRestartTracker;
use crate::context::CoreSupervisorStrategyHandle;
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
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);
}

pub trait CoreSupervisorStrategy: Any + Send + Sync {
  fn handle_child_failure<'a>(
    &'a self,
    ctx: &'a dyn CoreSupervisorContext,
    supervisor: &'a dyn CoreSupervisor,
    _child: CorePid,
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

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);
}

#[derive(Debug, Clone, Copy, Default)]
struct CoreEscalateSupervisorStrategy;

impl CoreEscalateSupervisorStrategy {
  #[inline]
  fn new() -> Self {
    Self
  }
}

impl CoreSupervisorStrategy for CoreEscalateSupervisorStrategy {
  fn handle_child_failure<'a>(
    &'a self,
    _ctx: &'a dyn CoreSupervisorContext,
    supervisor: &'a dyn CoreSupervisor,
    _child: CorePid,
    tracker: &'a mut CoreRestartTracker,
    reason: ErrorReasonCore,
    message: MessageHandle,
  ) -> CoreSupervisorStrategyFuture<'a> {
    let reason_clone = reason;
    let message_clone = message.clone();
    Box::pin(async move {
      // Escalate to parent by default and reset restart statistics.
      supervisor.escalate(reason_clone, message_clone).await;
      tracker.reset();
    })
  }
}

/// コア層で利用するデフォルトのスーパーバイザ戦略。
/// 子アクターの障害は親へ即座にエスカレートし、再起動統計をリセットします。
#[must_use]
pub fn default_core_supervisor_strategy() -> CoreSupervisorStrategyHandle {
  Arc::new(CoreEscalateSupervisorStrategy::new())
}
