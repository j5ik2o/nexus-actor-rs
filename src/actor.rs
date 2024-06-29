use std::fmt::{Debug, Display};
use std::sync::Arc;

pub mod actor;
mod actor_context;
pub mod actor_process;
pub mod actor_system;
mod auto_respond;
pub mod behavior;
pub mod context;
pub mod dead_letter_process;
pub mod directive;
pub mod dispatcher;
#[cfg(test)]
mod dispatcher_test;
pub mod future;
pub mod guardians_value;
pub mod log;
pub mod mailbox;
mod mailbox_middleware;
pub mod message;
pub mod message_envelope;
mod message_invoker;
pub mod messages;
mod middleware_chain;
pub mod pid;
pub mod pid_set;
#[cfg(test)]
mod pid_set_test;
pub mod process;
pub mod process_registry;
pub mod props;
mod props_opts;
pub mod restart_statistics;
mod root_context;
mod strategy_on_for_one;
pub mod supervision_event;
pub mod supervisor_strategy;
pub mod taks;
pub mod throttler;
pub mod unbounded;

pub trait Reason: Debug + Display + Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Debug, Clone)]
pub struct ReasonHandle(Arc<dyn Reason>);

impl Display for ReasonHandle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0.to_string())
  }
}

impl ReasonHandle {
  pub fn new(reason: Arc<dyn Reason>) -> Self {
    ReasonHandle(reason)
  }
}

impl PartialEq for ReasonHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReasonHandle {}

impl std::hash::Hash for ReasonHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Reason).hash(state);
  }
}

impl Reason for ReasonHandle {
  fn as_any(&self) -> &dyn std::any::Any {
    self.0.as_any()
  }
}
