use alloc::boxed::Box;
use alloc::sync::Arc;
use core::marker::PhantomData;

use crate::failure::{FailureEvent, FailureInfo};
use crate::mailbox::{PriorityEnvelope, SystemMessage};
use crate::{MailboxRuntime, PriorityActorRef};
use nexus_utils_core_rs::{Element, QueueError};

/// FailureInfo をどのように上位へ伝達するかを制御するためのシンク。
pub trait EscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// `already_handled` が true の場合はローカルで処理済みであることを示す。
  /// 追加の通知だけ行いたい場合は、戻り値を `Ok(())` にする。
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo>;
}

/// 親 Guardian へ `SystemMessage::Escalate` を転送するシンク。
pub struct ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  control_ref: PriorityActorRef<M, R>,
  map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
}

impl<M, R> ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(control_ref: PriorityActorRef<M, R>, map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>) -> Self {
    Self {
      control_ref,
      map_system,
    }
  }
}

impl<M, R> EscalationSink<M, R> for ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo> {
    if already_handled {
      return Ok(());
    }

    if let Some(parent_info) = info.escalate_to_parent() {
      let envelope =
        PriorityEnvelope::from_system(SystemMessage::Escalate(parent_info.clone())).map(|sys| (self.map_system)(sys));
      if self.control_ref.sender().try_send(envelope).is_ok() {
        return Ok(());
      }
    } else {
      let envelope =
        PriorityEnvelope::from_system(SystemMessage::Escalate(info.clone())).map(|sys| (self.map_system)(sys));
      if self.control_ref.sender().try_send(envelope).is_ok() {
        return Ok(());
      }
    }

    Err(info)
  }
}

/// カスタムハンドラをベースにしたシンク。
pub struct CustomEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  handler: Box<dyn FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static>,
  _marker: PhantomData<R>,
}

impl<M, R> CustomEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new<F>(handler: F) -> Self
  where
    F: FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static, {
    Self {
      handler: Box::new(handler),
      _marker: PhantomData,
    }
  }
}

impl<M, R> EscalationSink<M, R> for CustomEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, _already_handled: bool) -> Result<(), FailureInfo> {
    if (self.handler)(&info).is_ok() {
      Ok(())
    } else {
      Err(info)
    }
  }
}

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
          if !handled {
            last_failure = unhandled;
          }
        }
      }
    }

    if let Some(custom) = self.custom.as_mut() {
      match custom.handle(info.clone(), handled) {
        Ok(()) => handled = true,
        Err(unhandled) => {
          if !handled {
            last_failure = unhandled;
          }
        }
      }
    }

    if let Some(root) = self.root.as_mut() {
      let _ = root.handle(last_failure.clone(), handled);
      handled = true;
    }

    if handled {
      Ok(())
    } else {
      Err(last_failure)
    }
  }
}

pub type FailureEventHandler = Arc<dyn Fn(&FailureInfo) + Send + Sync>;
pub type FailureEventListener = Arc<dyn Fn(FailureEvent) + Send + Sync>;

pub struct RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  event_handler: Option<FailureEventHandler>,
  event_listener: Option<FailureEventListener>,
  _marker: PhantomData<(M, R)>,
}

impl<M, R> RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new() -> Self {
    Self {
      event_handler: None,
      _marker: PhantomData,
      event_listener: None,
    }
  }

  pub fn set_event_handler(&mut self, handler: Option<FailureEventHandler>) {
    self.event_handler = handler;
  }

  pub fn set_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.event_listener = listener;
  }
}

impl<M, R> Default for RootEscalationSink<M, R>
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

impl<M, R> EscalationSink<M, R> for RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, _already_handled: bool) -> Result<(), FailureInfo> {
    #[cfg(feature = "std")]
    {
      use tracing::error;
      error!(actor = ?info.actor, reason = %info.reason, path = ?info.path.segments(), "actor escalation reached root guardian");
    }

    if let Some(handler) = self.event_handler.as_ref() {
      handler(&info);
    }

    if let Some(listener) = self.event_listener.as_ref() {
      listener(FailureEvent::RootEscalated(info.clone()));
    }

    Ok(())
  }
}
