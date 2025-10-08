use alloc::sync::Arc;

use crate::failure::FailureInfo;
use crate::mailbox::SystemMessage;
use crate::{MailboxRuntime, PriorityActorRef, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

use super::{
  CustomEscalationSink, EscalationSink, FailureEventHandler, FailureEventListener, ParentGuardianSink,
  RootEscalationSink,
};

/// 複数シンクを合成し、順番に適用する。
pub struct CompositeEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  parent_guardian: Option<ParentGuardianSink<M, R>>,
  custom: Option<CustomEscalationSink<M, R>>,
  root: Option<RootEscalationSink<M, R>>,
}

impl<M, R> CompositeEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new() -> Self {
    Self {
      parent_guardian: None,
      custom: None,
      root: Some(RootEscalationSink::default()),
    }
  }

  pub fn with_parent_guardian(
    control_ref: PriorityActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  ) -> Self {
    let mut sink = Self::new();
    sink.set_parent_guardian(control_ref, map_system);
    sink
  }

  pub fn set_parent_guardian(
    &mut self,
    control_ref: PriorityActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  ) {
    self.parent_guardian = Some(ParentGuardianSink::new(control_ref, map_system));
  }

  pub fn clear_parent_guardian(&mut self) {
    self.parent_guardian = None;
  }

  pub fn set_custom_handler<F>(&mut self, handler: F)
  where
    F: FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static, {
    self.custom = Some(CustomEscalationSink::new(handler));
  }

  pub fn clear_custom_handler(&mut self) {
    self.custom = None;
  }

  pub fn set_root_handler(&mut self, handler: Option<FailureEventHandler>) {
    if let Some(root) = self.root.as_mut() {
      root.set_event_handler(handler);
    } else {
      let mut sink = RootEscalationSink::default();
      sink.set_event_handler(handler);
      self.root = Some(sink);
    }
  }

  pub fn set_root_listener(&mut self, listener: Option<FailureEventListener>) {
    if let Some(root) = self.root.as_mut() {
      root.set_event_listener(listener);
    } else if let Some(listener) = listener {
      let mut sink = RootEscalationSink::default();
      sink.set_event_listener(Some(listener));
      self.root = Some(sink);
    }
  }
}

impl<M, R> Default for CompositeEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<M, R> EscalationSink<M, R> for CompositeEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo> {
    let mut handled = already_handled;
    let mut last_failure = info.clone();

    if let Some(parent) = self.parent_guardian.as_mut() {
      match parent.handle(info.clone(), handled) {
        Ok(()) => handled = true,
        Err(unhandled) => {
          last_failure = unhandled;
          handled = false;
        }
      }
    }

    if let Some(custom) = self.custom.as_mut() {
      match custom.handle(info.clone(), handled) {
        Ok(()) => handled = true,
        Err(unhandled) => {
          last_failure = unhandled;
          handled = false;
        }
      }
    }

    if let Some(root) = self.root.as_mut() {
      let _ = root.handle(last_failure.clone(), handled);
    }

    if handled {
      Ok(())
    } else {
      Err(last_failure)
    }
  }
}
