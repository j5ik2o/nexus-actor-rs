use crate::actor::actor::Pid;

#[derive(Debug, Clone, PartialEq)]
pub struct TerminateInfo {
    pub who: Option<Pid>,
    pub why: i32,
}