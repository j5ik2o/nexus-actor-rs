use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::context::{ActorContext, ChildSpawnSpec, PriorityActorRef};
use crate::supervisor::Supervisor;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueRw};

/// 優先度付きメールボックスを前提とした単純なスケジューラ実装。
pub struct PriorityScheduler<M, R>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  runtime: R,
  actors: Vec<ActorCell<M, R>>,
}

impl<M, R> PriorityScheduler<M, R>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      runtime,
      actors: Vec::new(),
    }
  }

  pub fn spawn_actor<F, Sup>(
    &mut self,
    supervisor: Sup,
    options: MailboxOptions,
    handler: F,
  ) -> PriorityActorRef<M, R>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    Sup: Supervisor<M>, {
    let (mailbox, sender) = self.runtime.build_mailbox::<PriorityEnvelope<M>>(options);
    let actor_sender = sender.clone();
    let handler_box: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static> =
      Box::new(handler);
    let cell = ActorCell::new(self.runtime.clone(), mailbox, sender, Box::new(supervisor), handler_box);
    self.actors.push(cell);
    PriorityActorRef::new(actor_sender)
  }

  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    let mut new_children = Vec::new();
    let len = self.actors.len();
    for idx in 0..len {
      let cell = &mut self.actors[idx];
      cell.process_all(&mut new_children)?;
    }
    self.actors.extend(new_children.into_iter());
    Ok(())
  }

  pub fn actor_count(&self) -> usize {
    self.actors.len()
  }
}

struct ActorCell<M, R>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  runtime: R,
  mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  supervisor: Box<dyn Supervisor<M>>,
  handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
}

impl<M, R> ActorCell<M, R>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn new(
    runtime: R,
    mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    supervisor: Box<dyn Supervisor<M>>,
    handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
  ) -> Self {
    Self {
      runtime,
      mailbox,
      sender,
      supervisor,
      handler,
    }
  }

  fn process_all(&mut self, new_children: &mut Vec<ActorCell<M, R>>) -> Result<usize, QueueError<PriorityEnvelope<M>>> {
    let mut drained = Vec::new();
    while let Some(envelope) = self.mailbox.queue().poll()? {
      drained.push(envelope);
    }

    drained.sort_by(|a, b| b.priority().cmp(&a.priority()));

    let processed = drained.len();

    for envelope in drained.into_iter() {
      self.dispatch_envelope(envelope, new_children);
    }

    Ok(processed)
  }

  fn dispatch_envelope(&mut self, envelope: PriorityEnvelope<M>, new_children: &mut Vec<ActorCell<M, R>>) {
    let (message, priority) = envelope.into_parts();
    self.supervisor.before_handle();
    let mut pending_specs = Vec::new();
    {
      let mut ctx = ActorContext::new(
        &self.runtime,
        &self.sender,
        self.supervisor.as_mut(),
        &mut pending_specs,
      );
      ctx.enter_priority(priority);
      (self.handler)(&mut ctx, message);
      ctx.exit_priority();
    }
    self.supervisor.after_handle();
    for spec in pending_specs.into_iter() {
      self.register_child_from_spec(spec, new_children);
    }
  }

  fn register_child_from_spec(&mut self, spec: ChildSpawnSpec<M, R>, new_children: &mut Vec<ActorCell<M, R>>) {
    let ChildSpawnSpec {
      mailbox,
      sender,
      supervisor,
      handler,
    } = spec;

    let cell = ActorCell::new(self.runtime.clone(), mailbox, sender, supervisor, handler);
    new_children.push(cell);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use crate::mailbox::{MailboxOptions, SystemMessage};
  use nexus_utils_core_rs::DEFAULT_PRIORITY;
  use crate::supervisor::NoopSupervisor;
  use alloc::rc::Rc;
  use alloc::vec::Vec;
  use core::cell::RefCell;

  #[derive(Debug, Clone, PartialEq, Eq)]
  enum Message {
    User(u32),
    System(SystemMessage),
  }

  impl nexus_utils_core_rs::Element for Message {}

  #[test]
  fn scheduler_dispatches_high_priority_first() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<(u32, i8)>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let actor_ref = scheduler.spawn_actor(NoopSupervisor, MailboxOptions::default(), move |ctx, msg: u32| {
      log_clone.borrow_mut().push((msg, ctx.current_priority().unwrap()));
      if msg == 99 {
        let child_log = log_clone.clone();
        ctx
          .spawn_child(NoopSupervisor, MailboxOptions::default(), move |_, child_msg: u32| {
            child_log.borrow_mut().push((child_msg, 0));
          })
          .try_send_with_priority(7, 0)
          .unwrap();
      }
    });

    actor_ref.try_send_with_priority(10, 1).unwrap();
    actor_ref.try_send_with_priority(99, 7).unwrap();
    actor_ref.try_send_with_priority(20, 3).unwrap();

    scheduler.dispatch_all().unwrap();
    scheduler.dispatch_all().unwrap();

    assert_eq!(scheduler.actor_count(), 2);

    assert_eq!(log.borrow().as_slice(), &[(99, 7), (20, 3), (10, 1), (7, 0)]);
  }

  #[test]
  fn scheduler_prioritizes_system_messages() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let actor_ref = scheduler.spawn_actor(NoopSupervisor, MailboxOptions::default(), move |_, msg: Message| {
      log_clone.borrow_mut().push(msg.clone());
    });

    actor_ref
      .try_send_with_priority(Message::User(42), DEFAULT_PRIORITY)
      .unwrap();

    let control_envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(Message::System);
    actor_ref.try_send_envelope(control_envelope).unwrap();

    scheduler.dispatch_all().unwrap();

    assert_eq!(log.borrow().as_slice(), &[Message::System(SystemMessage::Stop), Message::User(42)]);
  }

  #[test]
  fn priority_actor_ref_sends_system_messages() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler: PriorityScheduler<SystemMessage, _> = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<SystemMessage>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let actor_ref = scheduler.spawn_actor(NoopSupervisor, MailboxOptions::default(), move |_, msg: SystemMessage| {
      log_clone.borrow_mut().push(msg.clone());
    });

    actor_ref.try_send_system(SystemMessage::Restart).unwrap();
    scheduler.dispatch_all().unwrap();

    assert_eq!(log.borrow().as_slice(), &[SystemMessage::Restart]);
  }
}
