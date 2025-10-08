use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::supervisor::Supervisor;
use crate::ActorId;
use crate::ActorPath;
use crate::SystemMessage;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueSize};

use super::{ChildSpawnSpec, InternalActorRef};

/// アクターが自身や子アクターを操作するためのコンテキスト。
pub struct ActorContext<'a, M, R, Sup>
where
  M: Element,
  R: MailboxRuntime,
  Sup: Supervisor<M> + ?Sized, {
  runtime: &'a R,
  sender: &'a QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  supervisor: &'a mut Sup,
  #[allow(dead_code)]
  pending_spawns: &'a mut Vec<ChildSpawnSpec<M, R>>,
  #[allow(dead_code)]
  map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  actor_path: ActorPath,
  actor_id: ActorId,
  watchers: &'a mut Vec<ActorId>,
  current_priority: Option<i8>,
  _marker: PhantomData<M>,
}

impl<'a, M, R, Sup> ActorContext<'a, M, R, Sup>
where
  M: Element,
  R: MailboxRuntime,
  Sup: Supervisor<M> + ?Sized,
{
  pub fn new(
    runtime: &'a R,
    sender: &'a QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    supervisor: &'a mut Sup,
    pending_spawns: &'a mut Vec<ChildSpawnSpec<M, R>>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
    actor_path: ActorPath,
    actor_id: ActorId,
    watchers: &'a mut Vec<ActorId>,
  ) -> Self {
    Self {
      runtime,
      sender,
      supervisor,
      pending_spawns,
      map_system,
      actor_path,
      actor_id,
      watchers,
      current_priority: None,
      _marker: PhantomData,
    }
  }

  pub fn runtime(&self) -> &R {
    self.runtime
  }

  pub fn supervisor(&mut self) -> &mut Sup {
    self.supervisor
  }

  pub fn actor_id(&self) -> ActorId {
    self.actor_id
  }

  pub fn actor_path(&self) -> &ActorPath {
    &self.actor_path
  }

  pub fn watchers(&self) -> &[ActorId] {
    self.watchers.as_slice()
  }

  pub fn register_watcher(&mut self, watcher: ActorId) {
    if !self.watchers.contains(&watcher) {
      self.watchers.push(watcher);
    }
  }

  pub fn unregister_watcher(&mut self, watcher: ActorId) {
    if let Some(index) = self.watchers.iter().position(|w| *w == watcher) {
      self.watchers.swap_remove(index);
    }
  }

  #[allow(dead_code)]
  pub(crate) fn spawn_child<F, S>(
    &mut self,
    supervisor: S,
    options: MailboxOptions,
    handler: F,
  ) -> InternalActorRef<M, R>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    S: Supervisor<M> + 'static, {
    let (mailbox, sender) = self.runtime.build_mailbox::<PriorityEnvelope<M>>(options);
    let actor_ref = InternalActorRef::new(sender.clone());
    let watchers = vec![self.actor_id];
    self.pending_spawns.push(ChildSpawnSpec {
      mailbox,
      sender,
      supervisor: Box::new(supervisor),
      handler: Box::new(handler),
      watchers,
      map_system: self.map_system.clone(),
      parent_path: self.actor_path.clone(),
    });
    actor_ref
  }

  #[allow(dead_code)]
  pub(crate) fn spawn_control_child<F, S>(&mut self, supervisor: S, handler: F) -> InternalActorRef<M, R>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    S: Supervisor<M> + 'static, {
    let options = MailboxOptions::default().with_priority_capacity(QueueSize::limitless());
    self.spawn_child(supervisor, options, handler)
  }

  pub fn current_priority(&self) -> Option<i8> {
    self.current_priority
  }

  pub fn send_to_self_with_priority(&self, message: M, priority: i8) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(PriorityEnvelope::new(message, priority))
  }

  pub fn send_control_to_self(&self, message: M, priority: i8) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(PriorityEnvelope::control(message, priority))
  }

  pub fn send_envelope_to_self(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(envelope)
  }

  pub(crate) fn enter_priority(&mut self, priority: i8) {
    self.current_priority = Some(priority);
  }

  pub(crate) fn exit_priority(&mut self) {
    self.current_priority = None;
  }
}
