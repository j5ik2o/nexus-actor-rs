use crate::actor::actor::Pid;
use crate::actor::message::terminate_reason::TerminateReason;

#[derive(Debug, Clone, PartialEq)]
pub struct TerminateInfo {
  pub who: Option<Pid>,
  pub why: TerminateReason,
}
