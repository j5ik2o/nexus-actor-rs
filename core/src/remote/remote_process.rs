use crate::actor::actor::ExtendedPid;
use crate::actor::message::{unwrap_envelope, MessageHandle, ReadonlyMessageHeadersHandle, SystemMessage};
use crate::actor::process::Process;
use crate::generated::actor::{Pid, Unwatch, Watch};
use crate::remote::messages::{RemoteUnwatch, RemoteWatch};
use crate::remote::remote::Remote;
use crate::remote::serializer::SerializerId;
use async_trait::async_trait;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct RemoteProcess {
  pid: Pid,
  remote: Remote,
}

impl RemoteProcess {
  pub fn new(pid: Pid, remote: Remote) -> Self {
    Self { pid, remote }
  }

  pub fn get_pid(&self) -> &Pid {
    &self.pid
  }

  pub fn get_pid_mut(&mut self) -> &mut Pid {
    &mut self.pid
  }

  pub fn get_remote(&self) -> &Remote {
    &self.remote
  }

  pub fn get_remote_mut(&mut self) -> &mut Remote {
    &mut self.remote
  }
}

#[async_trait]
impl Process for RemoteProcess {
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message_handle: MessageHandle) {
    tracing::debug!("Sending user message to remote process");
    let (header_opt, msg, sender_opt) = unwrap_envelope(message_handle);
    let pid = pid.cloned().expect("not found").inner_pid;

    let header_opt = header_opt.map(|h| ReadonlyMessageHeadersHandle::new(h));
    let sender_opt = sender_opt.map(|e| e.inner_pid.clone());
    self
      .remote
      .send_message(pid, header_opt, msg, sender_opt, SerializerId::None)
      .await;
  }

  async fn send_system_message(&self, pid: &ExtendedPid, message_handle: MessageHandle) {
    tracing::debug!("Sending system message to remote process");
    let watch_opt = message_handle.to_typed::<Watch>();
    let unwatch_opt = message_handle.to_typed::<Unwatch>();
    match (watch_opt, unwatch_opt) {
      (Some(watch), None) => {
        let rd = RemoteWatch {
          watcher: watch.watcher.unwrap(),
          watchee: pid.inner_pid.clone(),
        };
        self.remote.get_endpoint_manager().await.remote_watch(rd).await;
      }
      (None, Some(unwatch)) => {
        let ruw = RemoteUnwatch {
          watcher: unwatch.watcher.unwrap(),
          watchee: pid.inner_pid.clone(),
        };
        self.remote.get_endpoint_manager().await.remote_unwatch(ruw).await;
      }
      (_, _) => {
        self
          .remote
          .send_message(pid.inner_pid.clone(), None, message_handle, None, SerializerId::None)
          .await;
      }
    }
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self
      .send_system_message(pid, MessageHandle::new(SystemMessage::Stop))
      .await;
  }

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}
