use alloc::boxed::Box;
use alloc::sync::Arc;
use core::marker::PhantomData;

use crate::failure::FailureInfo;
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

    if handled {
      Ok(())
    } else {
      Err(last_failure)
    }
  }
}
