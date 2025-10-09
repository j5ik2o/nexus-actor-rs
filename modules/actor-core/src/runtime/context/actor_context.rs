use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::ActorId;
use crate::ActorPath;
use crate::Supervisor;
use crate::{MailboxFactory, MailboxOptions, PriorityEnvelope, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueSize};

use crate::runtime::scheduler::ReceiveTimeoutScheduler;
use core::cell::RefCell;
use core::time::Duration;

use super::{ChildSpawnSpec, InternalActorRef};
use crate::runtime::system::InternalProps;
use crate::MapSystemShared;

pub type ActorHandlerFn<M, R> = dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static;
/// Context for actors to operate on themselves and child actors.
pub struct ActorContext<'a, M, R, Sup>
where
  M: Element,
  R: MailboxFactory,
  Sup: Supervisor<M> + ?Sized, {
  runtime: &'a R,
  sender: &'a QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  supervisor: &'a mut Sup,
  #[allow(dead_code)]
  pending_spawns: &'a mut Vec<ChildSpawnSpec<M, R>>,
  #[allow(dead_code)]
  map_system: MapSystemShared<M>,
  actor_path: ActorPath,
  actor_id: ActorId,
  watchers: &'a mut Vec<ActorId>,
  current_priority: Option<i8>,
  receive_timeout: Option<&'a RefCell<Box<dyn ReceiveTimeoutScheduler>>>,
  _marker: PhantomData<M>,
}

impl<'a, M, R, Sup> ActorContext<'a, M, R, Sup>
where
  M: Element,
  R: MailboxFactory,
  Sup: Supervisor<M> + ?Sized,
{
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    runtime: &'a R,
    sender: &'a QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    supervisor: &'a mut Sup,
    pending_spawns: &'a mut Vec<ChildSpawnSpec<M, R>>,
    map_system: MapSystemShared<M>,
    actor_path: ActorPath,
    actor_id: ActorId,
    watchers: &'a mut Vec<ActorId>,
    receive_timeout: Option<&'a RefCell<Box<dyn ReceiveTimeoutScheduler>>>,
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
      receive_timeout,
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

  pub(crate) fn self_ref(&self) -> InternalActorRef<M, R>
  where
    R::Queue<PriorityEnvelope<M>>: Clone,
    R::Signal: Clone, {
    InternalActorRef::new(self.sender.clone())
  }

  #[allow(dead_code)]
  fn enqueue_spawn(
    &mut self,
    supervisor: Box<dyn Supervisor<M>>,
    options: MailboxOptions,
    map_system: MapSystemShared<M>,
    handler: Box<ActorHandlerFn<M, R>>,
  ) -> InternalActorRef<M, R> {
    let (mailbox, sender) = self.runtime.build_mailbox::<PriorityEnvelope<M>>(options);
    let actor_ref = InternalActorRef::new(sender.clone());
    let watchers = vec![self.actor_id];
    self.pending_spawns.push(ChildSpawnSpec {
      mailbox,
      sender,
      supervisor,
      handler,
      watchers,
      map_system,
      parent_path: self.actor_path.clone(),
    });
    actor_ref
  }

  pub(crate) fn spawn_child<F, S>(
    &mut self,
    supervisor: S,
    options: MailboxOptions,
    handler: F,
  ) -> InternalActorRef<M, R>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    S: Supervisor<M> + 'static, {
    self.enqueue_spawn(
      Box::new(supervisor),
      options,
      self.map_system.clone(),
      Box::new(handler),
    )
  }

  pub(crate) fn spawn_child_from_props(
    &mut self,
    supervisor: Box<dyn Supervisor<M>>,
    props: InternalProps<M, R>,
  ) -> InternalActorRef<M, R>
  where
    R: MailboxFactory + Clone + 'static, {
    let InternalProps {
      options,
      map_system,
      handler,
    } = props;
    self.enqueue_spawn(supervisor, options, map_system, handler)
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

  pub fn has_receive_timeout_scheduler(&self) -> bool {
    self.receive_timeout.is_some()
  }

  pub fn set_receive_timeout(&mut self, duration: Duration) -> bool {
    if let Some(cell) = self.receive_timeout {
      cell.borrow_mut().set(duration);
      true
    } else {
      false
    }
  }

  pub fn cancel_receive_timeout(&mut self) -> bool {
    if let Some(cell) = self.receive_timeout {
      cell.borrow_mut().cancel();
      true
    } else {
      false
    }
  }

  pub(crate) fn notify_receive_timeout_activity(&mut self, influence: bool) {
    if !influence {
      return;
    }

    if let Some(cell) = self.receive_timeout {
      cell.borrow_mut().notify_activity();
    }
  }
}
