#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::Any;

use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::core_types::pid::CorePid;
use crate::actor::core_types::process::ProcessFuture;

pub trait CoreActorContext: Any + Send + Sync {
  fn self_pid(&self) -> CorePid;
  fn sender_pid(&self) -> Option<CorePid>;
  fn message(&self) -> Option<MessageHandle>;
  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle>;
  // TODO(#core-context): provide accessors for receive timeout / actor state snapshots once core API stabilises.
}

pub type CorePropsFactory = Box<dyn Fn() -> CoreProps + Send + Sync>;
pub type CoreMailboxFactory = Arc<dyn Fn() -> ProcessFuture<'static> + Send + Sync>;

#[derive(Clone, Default)]
pub struct CoreProps {
  pub actor_type: Option<&'static str>,
  pub mailbox_factory: Option<CoreMailboxFactory>,
  // TODO(#core-context): extend with minimal actor factory / supervisor hooks required by core Actors.
}
