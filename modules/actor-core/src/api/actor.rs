//! Actor API aggregation module.
//!
//! Re-exports basic types used by the runtime layer such as
//! [`SystemMessage`] and [`PriorityEnvelope`] from this module.

mod actor_ref;
mod ask;
mod behavior;
mod context;
mod props;
mod root_context;
mod system;
mod system_support;
#[cfg(test)]
mod tests;

pub use crate::runtime::mailbox::{
  Mailbox, MailboxFactory, MailboxOptions, MailboxPair, MailboxSignal, PriorityEnvelope, QueueMailbox,
  QueueMailboxProducer, QueueMailboxRecv, SystemMessage,
};
pub use crate::runtime::message::DynMessage as RuntimeMessage;
pub use actor_ref::ActorRef;
pub use ask::{ask_with_timeout, AskError, AskFuture, AskResult, AskTimeoutFuture};
pub use behavior::{ActorAdapter, Behavior, BehaviorDirective, Behaviors, SupervisorStrategy};
pub use context::{Context, ContextLogLevel, ContextLogger, MessageAdapterRef, SetupContext};
pub use props::Props;
pub use root_context::RootContext;
pub use system::{ActorSystem, ActorSystemConfig, ActorSystemRunner, ShutdownToken};
pub use system_support::{ActorSystemHandles, ActorSystemParts, Spawn, Timer};

#[doc(hidden)]
mod __actor_doc_refs {
  use super::*;
  use crate::runtime::message::DynMessage;
  use nexus_utils_core_rs::Element;

  #[allow(dead_code)]
  pub fn _priority_envelope_marker<M: Element>() {
    let _ = core::mem::size_of::<PriorityEnvelope<DynMessage>>();
    let _ = core::mem::size_of::<PriorityEnvelope<M>>();
  }

  #[allow(dead_code)]
  pub fn _system_message_marker(message: SystemMessage) -> SystemMessage {
    message
  }
}
