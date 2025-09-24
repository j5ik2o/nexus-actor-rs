use crate::config::Config;
use crate::endpoint::Endpoint;
use crate::endpoint_watcher::EndpointWatcher;
use crate::endpoint_writer::EndpointWriter;
use crate::endpoint_writer_mailbox::EndpointWriterMailbox;
use crate::remote::Remote;
use async_trait::async_trait;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{BasePart, ContextHandle, MessagePart, SpawnerPart};
use nexus_actor_core_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid, Props, RestartStatistics};
use nexus_actor_core_rs::actor::dispatch::{MailboxHandle, MailboxProducer};
use nexus_actor_core_rs::actor::message::{MessageHandle, ResponseHandle};
use nexus_actor_core_rs::actor::supervisor::{
  Supervisor, SupervisorHandle, SupervisorStrategy, SupervisorStrategyHandle,
};
use std::any::Any;
use std::sync::Weak;

#[derive(Debug, Clone)]
pub struct EndpointSupervisor {
  remote: Weak<Remote>,
}

impl EndpointSupervisor {
  pub fn new(remote: Weak<Remote>) -> Self {
    tracing::debug!("EndpointSupervisor::new");
    EndpointSupervisor { remote }
  }

  async fn get_config(&self) -> Config {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_config()
      .clone()
  }

  fn endpoint_writer_mailbox_producer(&self) -> MailboxProducer {
    let cloned_remote = self.remote.clone();
    MailboxProducer::new(move || {
      let cloned_remote = cloned_remote.clone();
      async move {
        let config = cloned_remote
          .upgrade()
          .expect("Remote has been dropped")
          .get_config()
          .clone();
        let batch_size = config.get_endpoint_writer_batch_size().await;
        let queue_size = config.get_endpoint_writer_queue_size().await;
        MailboxHandle::new(EndpointWriterMailbox::new(
          cloned_remote.clone(),
          batch_size,
          queue_size,
        ))
      }
    })
  }

  pub async fn spawn_endpoint_writer(
    &self,
    remote: Weak<Remote>,
    address: String,
    mut ctx: ContextHandle,
  ) -> ExtendedPid {
    let config = self.get_config().await;
    let props = Props::from_async_actor_producer_with_opts(
      move |_| {
        let cloned_remote = remote.clone();
        let cloned_address = address.clone();
        let cloned_config = config.clone();
        async move { EndpointWriter::new(cloned_remote, cloned_address, cloned_config) }
      },
      [Props::with_mailbox_producer(self.endpoint_writer_mailbox_producer())],
    )
    .await;
    ctx.spawn(props).await
  }

  pub async fn spawn_endpoint_watcher(
    &self,
    remote: Weak<Remote>,
    address: String,
    mut ctx: ContextHandle,
  ) -> ExtendedPid {
    let props = Props::from_async_actor_producer_with_opts(
      move |_| {
        let cloned_remote = remote.clone();
        let cloned_address = address.clone();
        async move { EndpointWatcher::new(cloned_remote, cloned_address) }
      },
      [],
    )
    .await;
    ctx.spawn(props).await
  }
}

#[async_trait]
impl Actor for EndpointSupervisor {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    tracing::debug!("EndpointSupervisor::receive");
    let address_opt = context_handle.get_message_handle().await.to_typed::<String>();
    if address_opt.is_none() {
      return Err(ActorError::ReceiveError(ErrorReason::new("address not found", 0)));
    }
    tracing::debug!("address: {:?}", address_opt);
    let address = address_opt.unwrap();
    tracing::debug!("address: {:?}", address);
    tracing::debug!("Starting endpoint writer");
    let endpoint_writer_pid = self
      .spawn_endpoint_writer(self.remote.clone(), address.clone(), context_handle.clone())
      .await;
    tracing::debug!("endpoint_writer_pid: {:?}", endpoint_writer_pid);
    tracing::debug!("Starting endpoint watcher");
    let endpoint_watcher_pid = self
      .spawn_endpoint_watcher(self.remote.clone(), address.clone(), context_handle.clone())
      .await;
    tracing::debug!("endpoint_watcher_pid: {:?}", endpoint_watcher_pid);
    let e = Endpoint::new(endpoint_writer_pid.inner_pid, endpoint_watcher_pid.inner_pid);
    tracing::debug!("EndpointSupervisor::receive: {:?}", e);
    tracing::debug!("Responding to the request");
    context_handle.respond(ResponseHandle::new(e)).await;
    tracing::debug!("EndpointSupervisor::receive: done");
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    Some(SupervisorStrategyHandle::new(self.clone()))
  }
}

#[async_trait]
impl SupervisorStrategy for EndpointSupervisor {
  async fn handle_child_failure(
    &self,
    _actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    _rs: RestartStatistics,
    _reason: ErrorReason,
    _message_handle: MessageHandle,
  ) {
    tracing::debug!("EndpointSupervisor::handle_child_failure");
    supervisor.stop_children(&[child]).await;
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
