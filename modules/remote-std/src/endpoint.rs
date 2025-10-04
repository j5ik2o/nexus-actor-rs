use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::generated::actor::Pid;
use nexus_actor_std_rs::Message;

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
}
