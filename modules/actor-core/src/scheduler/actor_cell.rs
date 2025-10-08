use alloc::boxed::Box;
#[cfg(feature = "std")]
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::fmt;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::actor_id::ActorId;
use crate::actor_path::ActorPath;
use crate::context::{ActorContext, ChildSpawnSpec, InternalActorRef};
use crate::failure::FailureInfo;
use crate::guardian::{Guardian, GuardianStrategy};
use crate::mailbox::{Mailbox, SystemMessage};
use crate::supervisor::Supervisor;
use crate::{MailboxRuntime, PriorityEnvelope, QueueMailbox, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError, QueueRw};

pub(crate) struct ActorCell<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
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
  pub(crate) fn new(
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

  fn collect_envelopes(&mut self) -> Result<Vec<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    let mut drained = Vec::new();
    while let Some(envelope) = self.mailbox.queue().poll()? {
      drained.push(envelope);
    }
    if drained.len() > 1 {
      drained.sort_by_key(|b| core::cmp::Reverse(b.priority()));
    }
    Ok(drained)
  }

  fn process_envelopes(
    &mut self,
    envelopes: Vec<PriorityEnvelope<M>>,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
    escalations: &mut Vec<FailureInfo>,
  ) -> Result<usize, QueueError<PriorityEnvelope<M>>> {
    let mut processed = 0;
    for envelope in envelopes.into_iter() {
      self.dispatch_envelope(envelope, guardian, new_children, escalations)?;
      processed += 1;
    }
    Ok(processed)
  }

  pub(crate) fn process_pending(
    &mut self,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
    escalations: &mut Vec<FailureInfo>,
  ) -> Result<usize, QueueError<PriorityEnvelope<M>>> {
    let envelopes = self.collect_envelopes()?;
    if envelopes.is_empty() {
      return Ok(0);
    }
    self.process_envelopes(envelopes, guardian, new_children, escalations)
  }

  pub(crate) async fn wait_and_process(
    &mut self,
    guardian: &mut Guardian<M, R, Strat>,
    new_children: &mut Vec<ActorCell<M, R, Strat>>,
    escalations: &mut Vec<FailureInfo>,
  ) -> Result<usize, QueueError<PriorityEnvelope<M>>> {
    let first = match self.mailbox.recv().await {
      Ok(message) => message,
      Err(QueueError::Disconnected) => return Ok(0),
      Err(err) => return Err(err),
    };
    let mut envelopes = vec![first];
    envelopes.extend(self.collect_envelopes()?);
    if envelopes.len() > 1 {
      envelopes.sort_by_key(|b| core::cmp::Reverse(b.priority()));
    }
    self.process_envelopes(envelopes, guardian, new_children, escalations)
  }

  pub(crate) fn signal_clone(&self) -> R::Signal {
    self.mailbox.signal().clone()
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
      Ok(())
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

    let control_ref = InternalActorRef::new(sender.clone());
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
