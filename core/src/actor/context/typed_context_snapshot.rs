use std::marker::PhantomData;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::context_snapshot::ContextSnapshot;
use crate::actor::core::{ActorHandle, ExtendedPid, TypedExtendedPid};
use crate::actor::message::{Message, MessageHandle, ReadonlyMessageHeadersHandle};
use crate::actor::typed_context::TypedContextSyncView;

#[derive(Debug, Clone)]
pub struct TypedContextSnapshot<M: Message> {
  snapshot: ContextSnapshot,
  _marker: PhantomData<M>,
}

impl<M: Message> TypedContextSnapshot<M> {
  pub fn new(snapshot: ContextSnapshot) -> Self {
    Self {
      snapshot,
      _marker: PhantomData,
    }
  }

  pub fn snapshot(&self) -> &ContextSnapshot {
    &self.snapshot
  }

  pub fn into_snapshot(self) -> ContextSnapshot {
    self.snapshot
  }

  fn typed_pid_from(opt: Option<&ExtendedPid>) -> Option<TypedExtendedPid<M>> {
    opt.cloned().map(TypedExtendedPid::new)
  }
}

impl<M: Message> TypedContextSyncView<M> for TypedContextSnapshot<M> {
  fn actor_system_snapshot(&self) -> Option<ActorSystem> {
    self.snapshot.actor_system().cloned()
  }

  fn actor_snapshot(&self) -> Option<ActorHandle> {
    self.snapshot.actor().cloned()
  }

  fn parent_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    Self::typed_pid_from(self.snapshot.parent())
  }

  fn self_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    Self::typed_pid_from(self.snapshot.self_pid())
  }

  fn message_handle_snapshot(&self) -> Option<MessageHandle> {
    self.snapshot.message_handle().cloned()
  }

  fn message_snapshot(&self) -> Option<M>
  where
    M: Clone, {
    self.snapshot.message_handle().and_then(|handle| handle.to_typed::<M>())
  }

  fn message_header_snapshot(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.snapshot.message_header().cloned()
  }

  fn sender_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    Self::typed_pid_from(self.snapshot.sender())
  }
}
