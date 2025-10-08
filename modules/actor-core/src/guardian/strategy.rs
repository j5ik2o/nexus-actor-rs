use core::fmt;

use crate::actor_id::ActorId;
use crate::supervisor::SupervisorDirective;
use crate::MailboxRuntime;
use nexus_utils_core_rs::Element;

/// Supervisor 戦略。protoactor-go の Strategy に相当する。
pub trait GuardianStrategy<M, R>: Send + 'static
where
  M: Element,
  R: MailboxRuntime, {
  fn decide(&mut self, actor: ActorId, error: &dyn fmt::Debug) -> SupervisorDirective;
  fn before_start(&mut self, _actor: ActorId) {}
  fn after_restart(&mut self, _actor: ActorId) {}
}

/// 最も単純な戦略: 常に Restart を指示する。
#[derive(Clone, Copy, Debug, Default)]
pub struct AlwaysRestart;

impl<M, R> GuardianStrategy<M, R> for AlwaysRestart
where
  M: Element,
  R: MailboxRuntime,
{
  fn decide(&mut self, _actor: ActorId, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Restart
  }
}
