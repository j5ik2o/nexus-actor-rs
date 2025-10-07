use alloc::boxed::Box;
#[cfg(feature = "std")]
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::fmt;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::actor_id::ActorId;
use crate::context::{ActorContext, ChildSpawnSpec, PriorityActorRef};
use crate::guardian::{AlwaysRestart, Guardian, GuardianStrategy};
use crate::mailbox::SystemMessage;
use crate::supervisor::Supervisor;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueRw};

/// 優先度付きメールボックスを前提とした単純なスケジューラ実装。
pub struct PriorityScheduler<M, R, Strat = AlwaysRestart>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  runtime: R,
  guardian: Guardian<M, R, Strat>,
  actors: Vec<ActorCell<M, R, Strat>>,
}

impl<M, R> PriorityScheduler<M, R, AlwaysRestart>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      runtime: runtime.clone(),
      guardian: Guardian::new(AlwaysRestart),
      actors: Vec::new(),
    }
  }

  pub fn with_strategy<Strat>(runtime: R, strategy: Strat) -> PriorityScheduler<M, R, Strat>
  where
    Strat: GuardianStrategy<M, R>, {
    PriorityScheduler {
      runtime,
      guardian: Guardian::new(strategy),
      actors: Vec::new(),
    }
  }
}

impl<M, R, Strat> PriorityScheduler<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn spawn_actor<F, Sup>(
    &mut self,
    supervisor: Sup,
    options: MailboxOptions,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
    handler: F,
  ) -> Result<PriorityActorRef<M, R>, QueueError<PriorityEnvelope<M>>>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    Sup: Supervisor<M>, {
    let (mailbox, sender) = self.runtime.build_mailbox::<PriorityEnvelope<M>>(options);
    let actor_sender = sender.clone();
    let handler_box: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static> =
      Box::new(handler);
    let control_ref = PriorityActorRef::new(actor_sender.clone());
    let mut watchers = Vec::new();
    watchers.push(ActorId::ROOT);
    let primary_watcher = watchers.first().copied();
    let actor_id = self.guardian.register_child(control_ref.clone(), map_system.clone(), primary_watcher)?;
    let cell = ActorCell::new(
      actor_id,
      map_system,
      watchers,
      self.runtime.clone(),
      mailbox,
      sender,
      Box::new(supervisor),
      handler_box,
    );
    self.actors.push(cell);
    Ok(control_ref)
  }

  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    let mut new_children = Vec::new();
    let len = self.actors.len();
    for idx in 0..len {
      let cell = &mut self.actors[idx];
      cell.process_all(&mut self.guardian, &mut new_children)?;
    }
    self.actors.extend(new_children.into_iter());
    Ok(())
  }

  pub fn actor_count(&self) -> usize {
    self.actors.len()
  }
}

struct ActorCell<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  #[cfg_attr(not(feature = "std"), allow(dead_code))]
  actor_id: ActorId,
  map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  watchers: Vec<ActorId>,
  runtime: R,
  mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
  supervisor: Box<dyn Supervisor<M>>,
  handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
  _strategy: PhantomData<Strat>,
}

impl<M, R, Strat> ActorCell<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  fn new(
    actor_id: ActorId,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
    watchers: Vec<ActorId>,
    runtime: R,
    mailbox: QueueMailbox<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    supervisor: Box<dyn Supervisor<M>>,
    handler: Box<dyn for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static>,
  ) -> Self {
    Self {
      actor_id,
      map_system,
      watchers,
      runtime,
      mailbox,
      sender,
      supervisor,
      handler,
      _strategy: PhantomData,
    }
  }

  fn process_all(
    &mut self,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
  ) -> Result<usize, QueueError<PriorityEnvelope<M>>> {
    let mut drained = Vec::new();
    while let Some(envelope) = self.mailbox.queue().poll()? {
      drained.push(envelope);
    }

    drained.sort_by(|a, b| b.priority().cmp(&a.priority()));

    let processed = drained.len();

    for envelope in drained.into_iter() {
      self.dispatch_envelope(envelope, guardian, new_children)?;
    }

    Ok(processed)
  }

  fn dispatch_envelope(
    &mut self,
    envelope: PriorityEnvelope<M>,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    let (message, priority) = envelope.into_parts();
    self.supervisor.before_handle();
    let mut pending_specs = Vec::new();
    #[cfg(feature = "std")]
    let result = catch_unwind(AssertUnwindSafe(|| {
      let mut ctx = ActorContext::new(
        &self.runtime,
        &self.sender,
        self.supervisor.as_mut(),
        &mut pending_specs,
        self.map_system.clone(),
        self.actor_id,
        &mut self.watchers,
      );
      ctx.enter_priority(priority);
      (self.handler)(&mut ctx, message);
      ctx.exit_priority();
    }));

    #[cfg(not(feature = "std"))]
    {
      let mut ctx = ActorContext::new(
        &self.runtime,
        &self.sender,
        self.supervisor.as_mut(),
        &mut pending_specs,
        self.map_system.clone(),
        self.actor_id,
        &mut self.watchers,
      );
      ctx.enter_priority(priority);
      (self.handler)(&mut ctx, message);
      ctx.exit_priority();
      self.supervisor.after_handle();
      for spec in pending_specs.into_iter() {
        self.register_child_from_spec(spec, guardian, new_children)?;
      }
      return Ok(());
    }

    #[cfg(feature = "std")]
    {
      self.supervisor.after_handle();

      match result {
        Ok(()) => {
          for spec in pending_specs.into_iter() {
            self.register_child_from_spec(spec, guardian, new_children)?;
          }
          Ok(())
        }
        Err(payload) => {
          let panic_debug = PanicDebug::new(&payload);
          guardian.notify_failure(self.actor_id, &panic_debug)?;
          Ok(())
        }
      }
    }
  }

  fn register_child_from_spec(
    &mut self,
    spec: ChildSpawnSpec<M, R>,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    let ChildSpawnSpec {
      mailbox,
      sender,
      supervisor,
      handler,
      watchers,
      map_system,
    } = spec;

    let control_ref = PriorityActorRef::new(sender.clone());
    let primary_watcher = watchers.first().copied();
    let actor_id = guardian.register_child(control_ref, map_system.clone(), primary_watcher)?;
    let cell = ActorCell::new(
      actor_id,
      map_system,
      watchers,
      self.runtime.clone(),
      mailbox,
      sender,
      supervisor,
      handler,
    );
    new_children.push(cell);
    Ok(())
  }
}

#[cfg(feature = "std")]
struct PanicDebug<'a> {
  payload: &'a (dyn core::any::Any + Send),
}

#[cfg(feature = "std")]
impl<'a> PanicDebug<'a> {
  fn new(payload: &'a (dyn core::any::Any + Send)) -> Self {
    Self { payload }
  }
}

#[cfg(feature = "std")]
impl fmt::Debug for PanicDebug<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(s) = self.payload.downcast_ref::<&str>() {
      write!(f, "panic: {s}")
    } else if let Some(s) = self.payload.downcast_ref::<String>() {
      write!(f, "panic: {s}")
    } else {
      write!(f, "panic: unknown payload")
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use crate::mailbox::{MailboxOptions, SystemMessage};
  use crate::supervisor::NoopSupervisor;
  use alloc::rc::Rc;
  use alloc::sync::Arc;
  use alloc::vec;
  use alloc::vec::Vec;
  #[cfg(feature = "std")]
  use core::cell::Cell;
  use core::cell::RefCell;
  use nexus_utils_core_rs::DEFAULT_PRIORITY;

  #[derive(Debug, Clone, PartialEq, Eq)]
  enum Message {
    User(u32),
    System(SystemMessage),
  }

  impl nexus_utils_core_rs::Element for Message {}

  #[test]
  fn scheduler_delivers_watch_before_user_messages() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let _actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |_, msg: Message| {
          log_clone.borrow_mut().push(msg.clone());
        },
      )
      .unwrap();

    scheduler.dispatch_all().unwrap();

    assert_eq!(
      log.borrow().as_slice(),
      &[Message::System(SystemMessage::Watch(ActorId::ROOT))]
    );
  }

  #[test]
  fn actor_context_exposes_parent_watcher() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler = PriorityScheduler::new(runtime);

    let watchers_log: Rc<RefCell<Vec<Vec<ActorId>>>> = Rc::new(RefCell::new(Vec::new()));
    let watchers_clone = watchers_log.clone();

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |ctx, msg: Message| {
          let current_watchers = ctx.watchers().to_vec();
          watchers_clone.borrow_mut().push(current_watchers);
          match msg {
            Message::User(_) => {}
            Message::System(_) => {}
          }
        },
      )
      .unwrap();

    scheduler.dispatch_all().unwrap();
    assert_eq!(watchers_log.borrow().as_slice(), &[vec![ActorId::ROOT]]);

    actor_ref
      .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
      .unwrap();
    scheduler.dispatch_all().unwrap();

    assert_eq!(
      watchers_log.borrow().as_slice(),
      &[vec![ActorId::ROOT], vec![ActorId::ROOT]]
    );
  }

  #[test]
  fn scheduler_dispatches_high_priority_first() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<(u32, i8)>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |ctx, msg: Message| match msg {
          Message::User(value) => {
            log_clone.borrow_mut().push((value, ctx.current_priority().unwrap()));
            if value == 99 {
              let child_log = log_clone.clone();
              ctx
                .spawn_child(
                  NoopSupervisor,
                  MailboxOptions::default(),
                  move |_, child_msg: Message| {
                    if let Message::User(child_value) = child_msg {
                      child_log.borrow_mut().push((child_value, 0));
                    }
                  },
                )
                .try_send_with_priority(Message::User(7), 0)
                .unwrap();
            }
          }
          Message::System(_) => {}
        },
      )
      .unwrap();

    actor_ref.try_send_with_priority(Message::User(10), 1).unwrap();
    actor_ref.try_send_with_priority(Message::User(99), 7).unwrap();
    actor_ref.try_send_with_priority(Message::User(20), 3).unwrap();

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

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |_, msg: Message| {
          log_clone.borrow_mut().push(msg.clone());
        },
      )
      .unwrap();

    actor_ref
      .try_send_with_priority(Message::User(42), DEFAULT_PRIORITY)
      .unwrap();

    let control_envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(Message::System);
    actor_ref.try_send_envelope(control_envelope).unwrap();

    scheduler.dispatch_all().unwrap();

    assert_eq!(
      log.borrow().as_slice(),
      &[
        Message::System(SystemMessage::Stop),
        Message::System(SystemMessage::Watch(ActorId::ROOT)),
        Message::User(42),
      ]
    );
  }

  #[test]
  fn priority_actor_ref_sends_system_messages() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler: PriorityScheduler<SystemMessage, _> = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<SystemMessage>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| sys),
        move |_, msg: SystemMessage| {
          log_clone.borrow_mut().push(msg.clone());
        },
      )
      .unwrap();

    actor_ref.try_send_system(SystemMessage::Restart).unwrap();
    scheduler.dispatch_all().unwrap();

    assert_eq!(
      log.borrow().as_slice(),
      &[SystemMessage::Restart, SystemMessage::Watch(ActorId::ROOT)]
    );
  }

  #[cfg(feature = "std")]
  #[test]
  fn scheduler_notifies_guardian_and_restarts_on_panic() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler: PriorityScheduler<Message, _, AlwaysRestart> = PriorityScheduler::new(runtime);

    let log: Rc<RefCell<Vec<Message>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();
    let should_panic = Rc::new(Cell::new(true));
    let panic_flag = should_panic.clone();

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |_, msg: Message| {
          match msg {
            Message::System(SystemMessage::Watch(_)) => {
              // Watch メッセージは監視登録のみなのでログに残さない
            }
            Message::User(_) if panic_flag.get() => {
              panic_flag.set(false);
              panic!("boom");
            }
            _ => {
              log_clone.borrow_mut().push(msg.clone());
            }
          }
        },
      )
      .unwrap();

    actor_ref
      .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
      .unwrap();

    assert!(scheduler.dispatch_all().is_ok());
    assert!(log.borrow().is_empty());

    scheduler.dispatch_all().unwrap();

    assert_eq!(log.borrow().as_slice(), &[Message::System(SystemMessage::Restart)]);
    assert!(!should_panic.get());
  }
}
