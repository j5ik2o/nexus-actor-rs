use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::actor_id::ActorId;
use crate::actor_path::ActorPath;
use crate::mailbox::SystemMessage;
use crate::supervisor::Supervisor;
use crate::{MailboxRuntime, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::Element;

use super::ActorContext;

/// 子アクター生成時に必要となる情報。
pub struct ChildSpawnSpec<M, R>
where
  M: Element,
  R: MailboxRuntime, {
  pub mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  pub sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  pub supervisor: Box<dyn Supervisor<M>>,
  pub handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
  pub watchers: Vec<ActorId>,
  pub map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  pub parent_path: ActorPath,
}
