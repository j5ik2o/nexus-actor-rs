use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::fmt;

use crate::actor_id::ActorId;
use crate::actor_path::ActorPath;
use crate::context::InternalActorRef;
use crate::failure::FailureInfo;
use crate::mailbox::{PriorityEnvelope, SystemMessage};
use crate::supervisor::SupervisorDirective;
use crate::MailboxRuntime;
use nexus_utils_core_rs::{Element, QueueError};

use super::{ChildRecord, FailureReasonDebug, GuardianStrategy};

/// Guardian: 子アクター群を監督し、SystemMessage を送出する。
pub struct Guardian<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime,
  Strat: GuardianStrategy<M, R>,
{
  next_id: usize,
  pub(crate) children: BTreeMap<ActorId, ChildRecord<M, R>>,
  strategy: Strat,
  _marker: core::marker::PhantomData<M>,
}

impl<M, R, Strat> Guardian<M, R, Strat>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn new(strategy: Strat) -> Self {
    Self {
      next_id: 0,
      children: BTreeMap::new(),
      strategy,
      _marker: core::marker::PhantomData,
    }
  }

  pub fn register_child(
    &mut self,
    control_ref: InternalActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
    watcher: Option<ActorId>,
    parent_path: &ActorPath,
  ) -> Result<(ActorId, ActorPath), QueueError<PriorityEnvelope<M>>> {
    let id = ActorId(self.next_id);
    self.next_id += 1;
    self.strategy.before_start(id);
    let path = parent_path.push_child(id);
    self.children.insert(
      id,
      ChildRecord {
        control_ref: control_ref.clone(),
        map_system: map_system.clone(),
        watcher,
        path: path.clone(),
      },
    );

    if let Some(watcher_id) = watcher {
      let map_clone = map_system.clone();
      let envelope = PriorityEnvelope::from_system(SystemMessage::Watch(watcher_id)).map(move |sys| (map_clone)(sys));
      control_ref.sender().try_send(envelope)?;
    }

    Ok((id, path))
  }

  pub fn remove_child(&mut self, id: ActorId) -> Option<InternalActorRef<M, R>> {
    self.children.remove(&id).map(|record| {
      if let Some(watcher_id) = record.watcher {
        let map_clone = record.map_system.clone();
        let envelope =
          PriorityEnvelope::from_system(SystemMessage::Unwatch(watcher_id)).map(move |sys| (map_clone)(sys));
        let _ = record.control_ref.sender().try_send(envelope);
      }
      record.control_ref
    })
  }

  pub fn child_ref(&self, id: ActorId) -> Option<&InternalActorRef<M, R>> {
    self.children.get(&id).map(|record| &record.control_ref)
  }

  pub fn notify_failure(
    &mut self,
    actor: ActorId,
    error: &dyn fmt::Debug,
  ) -> Result<Option<FailureInfo>, QueueError<PriorityEnvelope<M>>> {
    let path = match self.children.get(&actor) {
      Some(record) => record.path.clone(),
      None => ActorPath::new().push_child(actor),
    };
    let failure = FailureInfo::from_error(actor, path, error);
    let directive = self.strategy.decide(actor, error);
    self.handle_directive(actor, failure, directive)
  }

  pub fn stop_child(&mut self, actor: ActorId) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    if let Some(record) = self.children.get(&actor) {
      let envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(|sys| (record.map_system)(sys));
      record.control_ref.sender().try_send(envelope)
    } else {
      Ok(())
    }
  }

  pub fn escalate_failure(
    &mut self,
    failure: FailureInfo,
  ) -> Result<Option<FailureInfo>, QueueError<PriorityEnvelope<M>>> {
    let actor = failure.actor;
    let directive = self.strategy.decide(actor, &FailureReasonDebug(&failure.reason));
    self.handle_directive(actor, failure, directive)
  }

  pub fn child_route(
    &self,
    actor: ActorId,
  ) -> Option<(InternalActorRef<M, R>, Arc<dyn Fn(SystemMessage) -> M + Send + Sync>)> {
    self
      .children
      .get(&actor)
      .map(|record| (record.control_ref.clone(), record.map_system.clone()))
  }

  fn handle_directive(
    &mut self,
    actor: ActorId,
    failure: FailureInfo,
    directive: SupervisorDirective,
  ) -> Result<Option<FailureInfo>, QueueError<PriorityEnvelope<M>>> {
    match directive {
      SupervisorDirective::Resume => Ok(None),
      SupervisorDirective::Stop => {
        if let Some(record) = self.children.get(&actor) {
          let envelope = PriorityEnvelope::from_system(SystemMessage::Stop).map(|sys| (record.map_system)(sys));
          record.control_ref.sender().try_send(envelope)?;
          Ok(None)
        } else {
          Ok(Some(failure))
        }
      }
      SupervisorDirective::Restart => {
        if let Some(record) = self.children.get(&actor) {
          let envelope = PriorityEnvelope::from_system(SystemMessage::Restart).map(|sys| (record.map_system)(sys));
          record.control_ref.sender().try_send(envelope)?;
          self.strategy.after_restart(actor);
          Ok(None)
        } else {
          Ok(Some(failure))
        }
      }
      SupervisorDirective::Escalate => {
        if let Some(parent_failure) = failure.escalate_to_parent() {
          Ok(Some(parent_failure))
        } else {
          Ok(Some(failure))
        }
      }
    }
  }
}
