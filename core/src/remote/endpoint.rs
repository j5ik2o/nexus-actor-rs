use crate::generated::actor::Pid;

#[derive(Debug, Clone)]
pub struct Endpoint {
  writer: Pid,
  watcher: Pid,
}

impl Endpoint {
  pub fn new(writer: Pid, watcher: Pid) -> Self {
    Endpoint { writer, watcher }
  }

  pub fn get_address(&self) -> Pid {
    self.writer.clone()
  }
}
