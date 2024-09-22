use crate::actor::message::Message;
use crate::generated::actor::Pid;
use crate::Message;

#[derive(Debug, Clone, PartialEq, Message)]
pub struct Endpoint {
  writer: Pid,
  watcher: Pid,
}

impl Endpoint {
  pub fn new(writer: Pid, watcher: Pid) -> Self {
    Endpoint { writer, watcher }
  }

  pub fn get_watcher(&self) -> Pid {
    self.watcher.clone()
  }

  pub fn get_writer(&self) -> Pid {
    self.writer.clone()
  }

  pub fn get_address(&self) -> String {
    self.watcher.address.clone()
  }
}
