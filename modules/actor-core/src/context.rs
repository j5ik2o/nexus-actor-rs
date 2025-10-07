use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::mailbox::SystemMessage;
use crate::supervisor::Supervisor;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueSize};

/// アクター参照。QueueMailboxProducer をラップし、メッセージ送信 API を提供する。
pub struct PriorityActorRef<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
}

impl<M, R> Clone for PriorityActorRef<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn clone(&self) -> Self {
    Self {
      sender: self.sender.clone(),
    }
  }
}

impl<M, R> PriorityActorRef<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>) -> Self {
    Self { sender }
  }

  pub fn try_send_with_priority(&self, message: M, priority: i8) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(PriorityEnvelope::new(message, priority))
  }

  pub fn try_send_control_with_priority(
    &self,
    message: M,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(PriorityEnvelope::control(message, priority))
  }

  pub fn try_send_envelope(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(envelope)
  }

  pub fn sender(&self) -> &QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal> {
    &self.sender
  }
}

impl<R> PriorityActorRef<SystemMessage, R>
where
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<SystemMessage>>: Clone,
  R::Signal: Clone,
{
  pub fn try_send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<SystemMessage>>> {
    self.sender.try_send(PriorityEnvelope::from_system(message))
  }
}

/// 子アクター生成時に必要となる情報。
pub struct ChildSpawnSpec<M, R>
where
  M: Element,
  R: MailboxRuntime, {
  pub mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  pub sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  pub supervisor: Box<dyn Supervisor<M>>,
  pub handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
}

/// アクターが自身や子アクターを操作するためのコンテキスト。
pub struct ActorContext<'a, M, R, Sup>
where
  M: Element,
  R: MailboxRuntime,
  Sup: Supervisor<M> + ?Sized, {
  runtime: &'a R,
  sender: &'a QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  supervisor: &'a mut Sup,
  pending_spawns: &'a mut Vec<ChildSpawnSpec<M, R>>,
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
  ) -> Self {
    Self {
      runtime,
      sender,
      supervisor,
      pending_spawns,
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

  pub fn spawn_child<F, S>(&mut self, supervisor: S, options: MailboxOptions, handler: F) -> PriorityActorRef<M, R>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    S: Supervisor<M> + 'static, {
    let (mailbox, sender) = self.runtime.build_mailbox::<PriorityEnvelope<M>>(options);
    let actor_ref = PriorityActorRef::new(sender.clone());
    self.pending_spawns.push(ChildSpawnSpec {
      mailbox,
      sender,
      supervisor: Box::new(supervisor),
      handler: Box::new(handler),
    });
    actor_ref
  }

  pub fn spawn_control_child<F, S>(&mut self, supervisor: S, handler: F) -> PriorityActorRef<M, R>
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
