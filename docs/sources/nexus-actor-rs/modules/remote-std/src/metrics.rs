use nexus_actor_std_rs::actor::context::ContextHandle;
use nexus_actor_std_rs::actor::core::ExtendedPid;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
struct SenderSnapshotCounters {
  hits: AtomicU64,
  misses: AtomicU64,
}

static SENDER_SNAPSHOT_COUNTERS: Lazy<SenderSnapshotCounters> = Lazy::new(SenderSnapshotCounters::default);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SenderSnapshotReport {
  pub hits: u64,
  pub misses: u64,
}

/// Try to retrieve the sender PID synchronously, and fall back to the core snapshot when necessary.
pub async fn record_sender_snapshot(context: &ContextHandle) -> Option<ExtendedPid> {
  if let Some(pid) = context.try_get_sender_core() {
    SENDER_SNAPSHOT_COUNTERS.hits.fetch_add(1, Ordering::Relaxed);
    return Some(ExtendedPid::from(pid));
  }

  SENDER_SNAPSHOT_COUNTERS.misses.fetch_add(1, Ordering::Relaxed);
  let core_snapshot = context.core_snapshot().await;
  core_snapshot.sender_pid_core().map(ExtendedPid::from_core)
}

#[allow(dead_code)]
pub fn sender_snapshot_report() -> SenderSnapshotReport {
  SenderSnapshotReport {
    hits: SENDER_SNAPSHOT_COUNTERS.hits.load(Ordering::Relaxed),
    misses: SENDER_SNAPSHOT_COUNTERS.misses.load(Ordering::Relaxed),
  }
}

#[allow(dead_code)]
pub fn reset_sender_snapshot_metrics() {
  SENDER_SNAPSHOT_COUNTERS.hits.store(0, Ordering::Relaxed);
  SENDER_SNAPSHOT_COUNTERS.misses.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::messages::RemoteTerminate;
  use async_trait::async_trait;
  use nexus_actor_core_rs::CorePid;
  use nexus_actor_std_rs::actor::actor_system::ActorSystem;
  use nexus_actor_std_rs::actor::context::{
    BasePart, Context, ContextHandle, CoreSenderPart, ExtensionContext, ExtensionPart, InfoPart, MessagePart,
    ReceiverContext, ReceiverPart, SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
  };
  use nexus_actor_std_rs::actor::core::{ActorError, ActorHandle, Continuer, ExtendedPid, Props, SpawnError};
  use nexus_actor_std_rs::actor::message::{
    MessageEnvelope, MessageHandle, ReadonlyMessageHeadersHandle, ResponseHandle,
  };
  use nexus_actor_std_rs::actor::process::actor_future::ActorFuture;
  use nexus_actor_std_rs::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
  use nexus_actor_std_rs::generated::actor::Pid;
  use std::any::Any;
  use std::fmt;
  use std::time::Duration;

  #[derive(Clone)]
  struct TestContext {
    system: ActorSystem,
    self_pid: ExtendedPid,
    sender_pid: ExtendedPid,
    message: MessageHandle,
  }

  impl fmt::Debug for TestContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_struct("TestContext").finish()
    }
  }

  impl TestContext {
    fn new(system: ActorSystem, self_pid: ExtendedPid, sender_pid: ExtendedPid, message: MessageHandle) -> Self {
      Self {
        system,
        self_pid,
        sender_pid,
        message,
      }
    }
  }

  impl ExtensionContext for TestContext {}

  #[async_trait]
  impl ExtensionPart for TestContext {
    async fn get(&mut self, _: ContextExtensionId) -> Option<ContextExtensionHandle> {
      None
    }

    async fn set(&mut self, _: ContextExtensionHandle) {}
  }

  impl SenderContext for TestContext {}

  #[async_trait]
  impl CoreSenderPart for TestContext {
    async fn get_sender_core(&self) -> Option<CorePid> {
      Some(self.sender_pid.to_core())
    }

    async fn send_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
      self.send(ExtendedPid::from(pid), message_handle).await
    }

    async fn request_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
      self.request(ExtendedPid::from(pid), message_handle).await
    }

    async fn request_with_custom_sender_core(&mut self, pid: CorePid, message_handle: MessageHandle, sender: CorePid) {
      self
        .request_with_custom_sender(ExtendedPid::from(pid), message_handle, ExtendedPid::from(sender))
        .await
    }

    async fn request_future_core(&self, pid: CorePid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
      self
        .request_future(ExtendedPid::from(pid), message_handle, timeout)
        .await
    }
  }

  #[async_trait]
  impl SenderPart for TestContext {
    async fn get_sender(&self) -> Option<ExtendedPid> {
      Some(self.sender_pid.clone())
    }

    async fn send(&mut self, _: ExtendedPid, _: MessageHandle) {}

    async fn request(&mut self, _: ExtendedPid, _: MessageHandle) {}

    async fn request_with_custom_sender(&mut self, _: ExtendedPid, _: MessageHandle, _: ExtendedPid) {}

    async fn request_future(&self, _: ExtendedPid, _: MessageHandle, _: Duration) -> ActorFuture {
      panic!("request_future not used in test")
    }
  }

  impl ReceiverContext for TestContext {}

  #[async_trait]
  impl ReceiverPart for TestContext {
    async fn receive(&mut self, _: MessageEnvelope) -> Result<(), ActorError> {
      Ok(())
    }
  }

  impl SpawnerContext for TestContext {}

  #[async_trait]
  impl SpawnerPart for TestContext {
    async fn spawn(&mut self, _: Props) -> ExtendedPid {
      panic!("spawn not used in test")
    }

    async fn spawn_prefix(&mut self, _: Props, _: &str) -> ExtendedPid {
      panic!("spawn_prefix not used in test")
    }

    async fn spawn_named(&mut self, _: Props, _: &str) -> Result<ExtendedPid, SpawnError> {
      panic!("spawn_named not used in test")
    }
  }

  #[async_trait]
  impl InfoPart for TestContext {
    async fn get_parent(&self) -> Option<ExtendedPid> {
      None
    }

    async fn get_self_opt(&self) -> Option<ExtendedPid> {
      Some(self.self_pid.clone())
    }

    async fn set_self(&mut self, _: ExtendedPid) {}

    async fn get_actor(&self) -> Option<ActorHandle> {
      None
    }

    async fn get_actor_system(&self) -> ActorSystem {
      self.system.clone()
    }
  }

  #[async_trait]
  impl MessagePart for TestContext {
    async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
      None
    }

    async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
      Some(self.message.clone())
    }

    async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
      None
    }
  }

  #[async_trait]
  impl BasePart for TestContext {
    fn as_any(&self) -> &dyn Any {
      self
    }

    async fn get_receive_timeout(&self) -> Duration {
      Duration::from_secs(0)
    }

    async fn get_children(&self) -> Vec<ExtendedPid> {
      vec![]
    }

    async fn respond(&self, _: ResponseHandle) {}

    async fn stash(&mut self) {}

    async fn un_stash_all(&mut self) -> Result<(), ActorError> {
      Ok(())
    }

    async fn watch(&mut self, _: &ExtendedPid) {}

    async fn unwatch(&mut self, _: &ExtendedPid) {}

    async fn set_receive_timeout(&mut self, _: &Duration) {}

    async fn cancel_receive_timeout(&mut self) {}

    async fn forward(&self, _: &ExtendedPid) {}

    async fn reenter_after(&self, _: ActorFuture, _: Continuer) {}
  }

  #[async_trait]
  impl StopperPart for TestContext {
    async fn stop(&mut self, _: &ExtendedPid) {}

    async fn stop_future_with_timeout(&mut self, _: &ExtendedPid, _: Duration) -> ActorFuture {
      panic!("stop_future_with_timeout not used in test")
    }

    async fn poison(&mut self, _: &ExtendedPid) {}

    async fn poison_future_with_timeout(&mut self, _: &ExtendedPid, _: Duration) -> ActorFuture {
      panic!("poison_future_with_timeout not used in test")
    }
  }

  impl Context for TestContext {}

  #[tokio::test]
  async fn record_sender_snapshot_uses_core_snapshot_fallback() -> Result<(), Box<dyn std::error::Error>> {
    let actor_system = ActorSystem::new().await?;
    let self_pid = ExtendedPid::new(Pid {
      address: "core".to_string(),
      id: "self".to_string(),
      request_id: 0,
    });
    let sender_pid = ExtendedPid::new(Pid {
      address: "core".to_string(),
      id: "sender".to_string(),
      request_id: 0,
    });
    let message = MessageHandle::new(RemoteTerminate {
      watcher: None,
      watchee: Some(Pid {
        address: "core".to_string(),
        id: "watchee".to_string(),
        request_id: 0,
      }),
    });

    let context = TestContext::new(actor_system, self_pid.clone(), sender_pid.clone(), message);
    let handle = ContextHandle::new(context);

    let baseline = sender_snapshot_report();
    let result = record_sender_snapshot(&handle).await;
    assert_eq!(result.unwrap().to_pid().id, sender_pid.to_pid().id);
    let report = sender_snapshot_report();
    assert_eq!(report.hits, baseline.hits);
    assert_eq!(report.misses, baseline.misses + 1);

    Ok(())
  }
}
