use crate::actor::message::terminate_reason::TerminateReason;
use crate::generated::actor::Pid;

#[derive(Debug, Clone, PartialEq)]
pub struct TerminateInfo {
  pub who: Option<Pid>,
  pub why: TerminateReason,
}
