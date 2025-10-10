use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::ActorId;
use crate::ActorPath;
use crate::Supervisor;
use crate::{MailboxFactory, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::Element;

use super::ActorHandlerFn;
use crate::MapSystemShared;

/// Information required when spawning child actors.
pub struct ChildSpawnSpec<M, R>
where
  M: Element,
  R: MailboxFactory, {
  pub mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  pub sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  pub supervisor: Box<dyn Supervisor<M>>,
  pub handler: Box<ActorHandlerFn<M, R>>,
  pub watchers: Vec<ActorId>,
  pub map_system: MapSystemShared<M>,
  pub parent_path: ActorPath,
}
