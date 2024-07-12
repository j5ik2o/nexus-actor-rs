use crate::actor::actor::Pid;

#[derive(Debug, Clone, PartialEq)]
pub struct Watch {
  pub watcher: Option<Pid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unwatch {
  pub watcher: Option<Pid>,
}
