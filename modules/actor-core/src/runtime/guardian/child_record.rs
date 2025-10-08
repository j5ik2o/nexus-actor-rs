use alloc::sync::Arc;
use core::fmt;

use crate::runtime::context::InternalActorRef;
use crate::ActorId;
use crate::ActorPath;
use crate::MailboxRuntime;
use crate::SystemMessage;
use nexus_utils_core_rs::Element;

pub(crate) struct FailureReasonDebug<'a>(pub(super) &'a str);

impl fmt::Debug for FailureReasonDebug<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.0)
  }
}

#[allow(dead_code)]
pub(crate) struct ChildRecord<M, R>
where
  M: Element,
  R: MailboxRuntime, {
  pub(super) control_ref: InternalActorRef<M, R>,
  pub(super) map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  pub(super) watcher: Option<ActorId>,
  pub(super) path: ActorPath,
}
