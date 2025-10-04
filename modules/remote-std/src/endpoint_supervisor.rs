use crate::config::Config;
use crate::endpoint::Endpoint;
use crate::endpoint_watcher::EndpointWatcher;
use crate::endpoint_writer::EndpointWriter;
use crate::endpoint_writer_mailbox::EndpointWriterMailbox;
use crate::metrics::record_sender_snapshot;
use crate::remote::Remote;
use crate::watch_registry::WatchRegistry;
use crate::TransportEndpoint;
use async_trait::async_trait;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid, Props};
use nexus_actor_std_rs::actor::dispatch::{MailboxHandle, MailboxProducer, MailboxSyncHandle};
use nexus_actor_std_rs::actor::message::ResponseHandle;
use nexus_actor_std_rs::actor::supervisor::{Directive, OneForOneStrategy, SupervisorStrategyHandle};
use std::sync::{Arc, Weak};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EndpointSupervisor {
  remote: Weak<Remote>,
}

impl EndpointSupervisor {
  pub fn new(remote: Weak<Remote>) -> Self {
    tracing::debug!("EndpointSupervisor::new");
    EndpointSupervisor { remote }
  }

  fn get_config(&self) -> Config {
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
        let snapshot_interval = config.get_endpoint_writer_queue_snapshot_interval().await;
        let mailbox = EndpointWriterMailbox::new(cloned_remote.clone(), batch_size, queue_size, snapshot_interval);
        let sync = MailboxSyncHandle::new(mailbox.clone());
        MailboxHandle::new_with_sync(mailbox, Some(sync))
      }
    })
  }

  pub async fn spawn_endpoint_writer(
    &self,
    remote: Weak<Remote>,
    address: String,
    mut ctx: ContextHandle,
  ) -> ExtendedPid {
    let config = self.get_config();
    let props = Props::from_async_actor_producer_with_opts(
      move |_| {
        let cloned_remote = remote.clone();
        let cloned_address = address.clone();
        let cloned_config = config.clone();
        async move {
          EndpointWriter::new(
            cloned_remote,
            cloned_address.clone(),
            cloned_config.clone(),
            TransportEndpoint::new(cloned_address.clone()),
          )
        }
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
    let mut registry = None;
    if let Some(remote_instance) = remote.upgrade() {
      if let Some(manager) = remote_instance.get_endpoint_manager_opt().await {
        if let Some(existing) = manager.watch_registry(&address) {
          registry = Some(existing);
        } else {
          let created = Arc::new(WatchRegistry::new());
          manager.register_watch_registry(&address, created.clone());
          registry = Some(created);
        }
      }
    }
    let registry = registry.unwrap_or_else(|| Arc::new(WatchRegistry::new()));
    if let Some(remote_instance) = remote.upgrade() {
      if let Some(manager) = remote_instance.get_endpoint_manager_opt().await {
        manager.register_watch_registry(&address, registry.clone());
      }
    }
    let props = Props::from_async_actor_producer_with_opts(
      move |_| {
        let cloned_remote = remote.clone();
        let cloned_address = address.clone();
        let registry = registry.clone();
        async move { EndpointWatcher::with_registry(cloned_remote, cloned_address, registry) }
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
    let _ = record_sender_snapshot(&context_handle).await;
    let message_handle = if let Some(handle) = context_handle.try_get_message_handle_opt() {
      handle
    } else {
      context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
    };
    let address_opt = message_handle.to_typed::<String>();
    if address_opt.is_none() {
      return Err(ActorError::ReceiveError(ErrorReason::new("address not found", 0)));
    }
    tracing::debug!("address: {:?}", address_opt);
    let address = address_opt.unwrap();
    tracing::debug!("address: {:?}", address);
    let core_snapshot = context_handle.core_snapshot().await;
    let self_pid_core = core_snapshot.self_pid_core();
    let sender_pid_core = core_snapshot.sender_pid_core();
    tracing::debug!(
      "EndpointSupervisor::receive core snapshot self={:?} sender={:?}",
      self_pid_core,
      sender_pid_core
    );
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
    let strategy = OneForOneStrategy::new(0, Duration::ZERO).with_decider(|_| async { Directive::Stop });
    Some(SupervisorStrategyHandle::new(strategy))
  }
}
