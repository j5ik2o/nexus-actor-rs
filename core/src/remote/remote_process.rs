use crate::generated::actor::Pid;
use crate::remote::remote::Remote;
use std::sync::Arc;

pub struct RemoteProcess {
  pid: Pid,
  remote: Arc<Remote>,
}
