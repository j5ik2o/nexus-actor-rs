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
use crate::actor_path::ActorPath;
use crate::context::{ActorContext, ChildSpawnSpec, PriorityActorRef};
use crate::failure::FailureInfo;
use crate::guardian::{AlwaysRestart, Guardian, GuardianStrategy};
use crate::mailbox::SystemMessage;
use crate::supervisor::Supervisor;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueRw};

struct ParentGuardianRoute<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  control_ref: PriorityActorRef<M, R>,
  map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
}

struct SchedulerEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  parent_guardian: Option<ParentGuardianRoute<M, R>>,
  custom_handler: Option<Box<dyn FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static>>,
}

impl<M, R> SchedulerEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn new() -> Self {
    Self {
      parent_guardian: None,
      custom_handler: None,
    }
  }

  fn handle<Strat>(&mut self, info: FailureInfo, guardian: &Guardian<M, R, Strat>) -> Result<(), FailureInfo>
  where
    Strat: GuardianStrategy<M, R>, {
    let mut handled = false;

    if let Some(parent_failure) = info.escalate_to_parent() {
      if !parent_failure.path.is_empty() {
        if let Some((parent_ref, map_system)) = guardian.child_route(parent_failure.actor) {
          let envelope =
            PriorityEnvelope::from_system(SystemMessage::Escalate(parent_failure.clone())).map(|sys| (map_system)(sys));
          if parent_ref.sender().try_send(envelope).is_ok() {
            handled = true;
          }
        }
      }

      if !handled {
        if let Some(route) = self.parent_guardian.as_ref() {
          let envelope = PriorityEnvelope::from_system(SystemMessage::Escalate(parent_failure.clone()))
            .map(|sys| (route.map_system)(sys));
          if route.control_ref.sender().try_send(envelope).is_ok() {
            handled = true;
          }
        }
      }
    } else if let Some(route) = self.parent_guardian.as_ref() {
      let envelope =
        PriorityEnvelope::from_system(SystemMessage::Escalate(info.clone())).map(|sys| (route.map_system)(sys));
      if route.control_ref.sender().try_send(envelope).is_ok() {
        handled = true;
      }
    }

    if let Some(handler) = self.custom_handler.as_mut() {
      if handler(&info).is_ok() {
        handled = true;
      }
    }

    if handled {
      Ok(())
    } else {
      Err(info)
    }
  }

  fn set_parent_guardian(
    &mut self,
    control_ref: PriorityActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  ) {
    self.parent_guardian = Some(ParentGuardianRoute {
      control_ref,
      map_system,
    });
  }

  fn set_custom_handler<F>(&mut self, handler: F)
  where
    F: FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static, {
    self.custom_handler = Some(Box::new(handler));
  }
}

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
  escalations: Vec<FailureInfo>,
  escalation_sink: SchedulerEscalationSink<M, R>,
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
      escalations: Vec::new(),
      escalation_sink: SchedulerEscalationSink::new(),
    }
  }

  pub fn with_strategy<Strat>(runtime: R, strategy: Strat) -> PriorityScheduler<M, R, Strat>
  where
    Strat: GuardianStrategy<M, R>, {
    PriorityScheduler {
      runtime,
      guardian: Guardian::new(strategy),
      actors: Vec::new(),
      escalations: Vec::new(),
      escalation_sink: SchedulerEscalationSink::new(),
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
    let parent_path = ActorPath::new();
    let (actor_id, actor_path) =
      self
        .guardian
        .register_child(control_ref.clone(), map_system.clone(), primary_watcher, &parent_path)?;
    let cell = ActorCell::new(
      actor_id,
      map_system,
      watchers,
      actor_path,
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
      cell.process_all(&mut self.guardian, &mut new_children, &mut self.escalations)?;
    }
    self.actors.extend(new_children.into_iter());

    if !self.escalations.is_empty() {
      let mut remaining = Vec::new();
      let guardian = &self.guardian;
      for info in self.escalations.drain(..) {
        match self.escalation_sink.handle(info, guardian) {
          Ok(()) => {}
          Err(unhandled) => remaining.push(unhandled),
        }
      }
      self.escalations.extend(remaining);
    }
    Ok(())
  }

  pub fn actor_count(&self) -> usize {
    self.actors.len()
  }

  pub fn take_escalations(&mut self) -> Vec<FailureInfo> {
    core::mem::take(&mut self.escalations)
  }

  pub fn on_escalation<F>(&mut self, handler: F)
  where
    F: FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static, {
    self.escalation_sink.set_custom_handler(handler);
  }

  pub fn set_parent_guardian(
    &mut self,
    control_ref: PriorityActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  ) {
    self.escalation_sink.set_parent_guardian(control_ref, map_system);
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
  actor_path: ActorPath,
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
    actor_path: ActorPath,
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
      actor_path,
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
    escalations: &mut Vec<FailureInfo>,
  ) -> Result<usize, QueueError<PriorityEnvelope<M>>> {
    let mut drained = Vec::new();
    while let Some(envelope) = self.mailbox.queue().poll()? {
      drained.push(envelope);
    }

    drained.sort_by(|a, b| b.priority().cmp(&a.priority()));

    let processed = drained.len();

    for envelope in drained.into_iter() {
      self.dispatch_envelope(envelope, guardian, new_children, escalations)?;
    }

    Ok(processed)
  }

  fn dispatch_envelope(
    &mut self,
    envelope: PriorityEnvelope<M>,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
    escalations: &mut Vec<FailureInfo>,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    if let Some(SystemMessage::Escalate(failure)) = envelope.system_message().cloned() {
      if let Some(next_failure) = guardian.escalate_failure(failure)? {
        escalations.push(next_failure);
      }
      return Ok(());
    }

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
        self.actor_path.clone(),
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
        self.actor_path.clone(),
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
          if let Some(info) = guardian.notify_failure(self.actor_id, &panic_debug)? {
            escalations.push(info);
          }
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
      parent_path,
    } = spec;

    let control_ref = PriorityActorRef::new(sender.clone());
    let primary_watcher = watchers.first().copied();
    let (actor_id, actor_path) =
      guardian.register_child(control_ref, map_system.clone(), primary_watcher, &parent_path)?;
    let cell = ActorCell::new(
      actor_id,
      map_system,
      watchers,
      actor_path,
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
  use crate::actor_id::ActorId;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use crate::mailbox::{MailboxOptions, SystemMessage};
  use crate::supervisor::NoopSupervisor;
  #[cfg(feature = "std")]
  use crate::SupervisorDirective;
  use alloc::rc::Rc;
  use alloc::sync::Arc;
  use alloc::vec;
  use alloc::vec::Vec;
  #[cfg(feature = "std")]
  use core::cell::Cell;
  use core::cell::RefCell;
  use nexus_utils_core_rs::DEFAULT_PRIORITY;

  #[cfg(feature = "std")]
  #[derive(Clone, Copy, Debug)]
  struct AlwaysEscalate;

  #[cfg(feature = "std")]
  impl<M, R> GuardianStrategy<M, R> for AlwaysEscalate
  where
    M: Element,
    R: MailboxRuntime,
  {
    fn decide(&mut self, _actor: ActorId, _error: &dyn core::fmt::Debug) -> SupervisorDirective {
      SupervisorDirective::Escalate
    }
  }

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

  #[cfg(feature = "std")]
  #[test]
  fn scheduler_records_escalations() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
      PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

    let sink: Rc<RefCell<Vec<FailureInfo>>> = Rc::new(RefCell::new(Vec::new()));
    let sink_clone = sink.clone();
    scheduler.on_escalation(move |info| {
      sink_clone.borrow_mut().push(info.clone());
      Ok(())
    });

    let should_panic = Rc::new(Cell::new(true));
    let panic_flag = should_panic.clone();

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |_, msg: Message| match msg {
          Message::System(SystemMessage::Watch(_)) => {}
          Message::User(_) if panic_flag.get() => {
            panic_flag.set(false);
            panic!("boom");
          }
          _ => {}
        },
      )
      .unwrap();

    actor_ref
      .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
      .unwrap();

    assert!(scheduler.dispatch_all().is_ok());

    let handler_data = sink.borrow();
    assert_eq!(handler_data.len(), 1);
    assert_eq!(handler_data[0].actor, ActorId(0));
    assert!(handler_data[0].reason.starts_with("panic:"));

    // handler で除去済みのため take_escalations は空
    assert!(scheduler.take_escalations().is_empty());
  }

  #[cfg(feature = "std")]
  #[test]
  fn scheduler_escalation_handler_delivers_to_parent() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
      PriorityScheduler::with_strategy(runtime.clone(), AlwaysEscalate);

    let (parent_mailbox, parent_sender) = runtime.build_default_mailbox::<PriorityEnvelope<Message>>();
    let parent_ref: PriorityActorRef<Message, TestMailboxRuntime> = PriorityActorRef::new(parent_sender);
    scheduler.set_parent_guardian(parent_ref, Arc::new(|sys| Message::System(sys)));

    let should_panic = Rc::new(Cell::new(true));
    let panic_flag = should_panic.clone();

    let actor_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |_, msg: Message| match msg {
          Message::System(SystemMessage::Watch(_)) => {}
          Message::User(_) if panic_flag.get() => {
            panic_flag.set(false);
            panic!("boom");
          }
          _ => {}
        },
      )
      .unwrap();

    actor_ref
      .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
      .unwrap();

    assert!(scheduler.dispatch_all().is_ok());

    let envelope = parent_mailbox.queue().poll().unwrap().unwrap();
    let (msg, _, channel) = envelope.into_parts_with_channel();
    assert_eq!(channel, crate::mailbox::PriorityChannel::Control);
    match msg {
      Message::System(SystemMessage::Escalate(info)) => {
        assert_eq!(info.actor, ActorId(0));
        assert!(info.reason.contains("panic"));
      }
      other => panic!("unexpected message: {:?}", other),
    }
  }

  #[cfg(feature = "std")]
  #[test]
  fn scheduler_escalation_chain_reaches_root() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut scheduler: PriorityScheduler<Message, _, AlwaysEscalate> =
      PriorityScheduler::with_strategy(runtime, AlwaysEscalate);

    let collected: Rc<RefCell<Vec<FailureInfo>>> = Rc::new(RefCell::new(Vec::new()));
    let collected_clone = collected.clone();
    scheduler.on_escalation(move |info| {
      collected_clone.borrow_mut().push(info.clone());
      Ok(())
    });

    let parent_triggered = Rc::new(Cell::new(false));
    let trigger_flag = parent_triggered.clone();
    let child_panics = Rc::new(Cell::new(true));
    let child_flag = child_panics.clone();

    let parent_ref = scheduler
      .spawn_actor(
        NoopSupervisor,
        MailboxOptions::default(),
        Arc::new(|sys| Message::System(sys)),
        move |ctx, msg: Message| match msg {
          Message::System(_) => {}
          Message::User(0) if !trigger_flag.get() => {
            trigger_flag.set(true);
            let panic_once = child_flag.clone();
            ctx
              .spawn_child(
                NoopSupervisor,
                MailboxOptions::default(),
                move |_, child_msg: Message| match child_msg {
                  Message::System(_) => {}
                  Message::User(1) if panic_once.get() => {
                    panic_once.set(false);
                    panic!("child failure");
                  }
                  _ => {}
                },
              )
              .try_send_with_priority(Message::User(1), DEFAULT_PRIORITY)
              .unwrap();
          }
          _ => {}
        },
      )
      .unwrap();

    scheduler.dispatch_all().unwrap();

    {
      let snapshot = collected.borrow();
      assert_eq!(snapshot.len(), 0);
    }

    parent_ref
      .try_send_with_priority(Message::User(0), DEFAULT_PRIORITY)
      .unwrap();

    scheduler.dispatch_all().unwrap();
    {
      let snapshot = collected.borrow();
      assert_eq!(snapshot.len(), 0);
    }

    scheduler.dispatch_all().unwrap();
    {
      let snapshot = collected.borrow();
      assert_eq!(snapshot.len(), 1);
    }
    let child_failure = collected.borrow()[0].clone();
    let parent_failure = child_failure
      .escalate_to_parent()
      .expect("parent failure info must exist");

    let root_failure = scheduler
      .guardian
      .escalate_failure(parent_failure.clone())
      .unwrap()
      .expect("root failure should be produced");

    assert_eq!(child_failure.path.segments().last().copied(), Some(child_failure.actor));

    assert_eq!(
      parent_failure.actor,
      child_failure
        .path
        .segments()
        .first()
        .copied()
        .unwrap_or(child_failure.actor)
    );

    assert_eq!(root_failure.actor, parent_failure.actor);
    assert!(root_failure.path.is_empty());
    assert_eq!(root_failure.reason, parent_failure.reason);
  }
}
