use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use nexus_utils_std_rs::concurrent::Synchronized;

use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::dispatch::MailboxMessage;
use crate::actor::message::Failure;
use crate::actor::message::MessageHandle;
use crate::actor::message::SystemMessage;
use crate::actor::process::{Process, ProcessHandle};
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::actor::supervisor::{StdSupervisorContext, Supervisor, SupervisorHandle};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;

#[derive(Debug, Clone)]
pub struct GuardiansValue {
  actor_system: WeakActorSystem,
  guardians: Arc<Synchronized<HashMap<SupervisorStrategyHandle, GuardianProcess>>>,
}

impl GuardiansValue {
  pub fn new(actor_system: ActorSystem) -> Self {
    GuardiansValue {
      actor_system: actor_system.downgrade(),
      guardians: Arc::new(Synchronized::new(HashMap::new())),
    }
  }

  fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before GuardiansValue")
  }

  pub async fn get_guardian_pid(&self, s: SupervisorStrategyHandle) -> ExtendedPid {
    let handle = s.clone();
    let res = self.guardians.read(|guardians| guardians.get(&handle).cloned()).await;
    match res {
      Some(guardian) => {
        let pid = guardian.pid.clone();
        match &*pid {
          Some(p) => p.clone(),
          None => {
            panic!("Guardian PID is not initialized");
          }
        }
      }
      None => {
        let guardian = GuardianProcess::new(Arc::new(self.clone()), s.clone()).await;
        self
          .guardians
          .write(|guardians| {
            guardians.insert(s.clone(), guardian.clone());
          })
          .await;
        let pid = guardian.pid.clone();
        match &*pid {
          Some(p) => p.clone(),
          None => {
            panic!("Guardian PID is not initialized");
          }
        }
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct GuardianProcess {
  guardians: Arc<GuardiansValue>,
  pid: Arc<Option<ExtendedPid>>,
  strategy: SupervisorStrategyHandle,
}

impl GuardianProcess {
  async fn new(guardians: Arc<GuardiansValue>, s: SupervisorStrategyHandle) -> GuardianProcess {
    let mut guardian = GuardianProcess {
      strategy: s,
      guardians: guardians.clone(),
      pid: Arc::new(None), // This should be properly initialized
    };
    let actor_system = guardians.actor_system();
    let id = actor_system.get_process_registry().await.next_id();
    let ph = ProcessHandle::new(guardian.clone());
    let (pid, ok) = actor_system
      .get_process_registry()
      .await
      .add_process(ph, &format!("guardian-{}", id))
      .await;
    if !ok {
      tracing::error!("failed to register guardian process: pid = {}", pid);
    }
    guardian.pid = Arc::new(Some(pid));
    guardian
  }
}

#[async_trait]
impl Process for GuardianProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, _: MessageHandle) {
    panic!("guardian actor cannot receive any user messages");
  }

  async fn send_system_message(&self, _: &ExtendedPid, message_handle: MessageHandle) {
    if let Some(failure) = message_handle.to_typed::<Failure>() {
      let actor_system = self.guardians.actor_system();
      let supervisor_clone = self.clone();
      let supervisor_arc: Arc<dyn Supervisor> = Arc::new(supervisor_clone);
      let supervisor_handle =
        SupervisorHandle::new_arc_with_metrics(supervisor_arc.clone(), actor_system.metrics_runtime_slot());
      supervisor_handle.inject_snapshot(supervisor_arc);
      let core_context = StdSupervisorContext::new(actor_system.clone());
      let core_supervisor = supervisor_handle.core_adapter();
      let mut tracker = failure.restart_stats.to_core_tracker().await;
      let reason_core = failure.reason.as_core().clone();
      let message_handle = failure.message_handle.clone();

      self
        .strategy
        .core_strategy()
        .handle_child_failure(
          &core_context,
          &core_supervisor,
          failure.who.to_core(),
          &mut tracker,
          reason_core,
          message_handle,
        )
        .await;

      failure.restart_stats.overwrite_with(tracker).await;
    }
  }

  async fn stop(&self, _pid: &ExtendedPid) {
    // Ignore
  }

  fn set_dead(&self) {
    // Ignore
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl Supervisor for GuardianProcess {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }

  async fn get_children(&self) -> Vec<CorePid> {
    panic!("guardian does not hold its children PIDs");
  }

  async fn escalate_failure(&self, _: ErrorReason, _: MessageHandle) {
    panic!("guardian cannot escalate failure");
  }

  async fn restart_children(&self, pids: &[CorePid]) {
    let actor_system = self.guardians.actor_system();
    for pid in ExtendedPid::from_core_slice(pids) {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(SystemMessage::Restart))
        .await;
    }
  }

  async fn stop_children(&self, pids: &[CorePid]) {
    let actor_system = self.guardians.actor_system();
    for pid in ExtendedPid::from_core_slice(pids) {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(SystemMessage::Stop))
        .await;
    }
  }

  async fn resume_children(&self, pids: &[CorePid]) {
    let actor_system = self.guardians.actor_system();
    for pid in ExtendedPid::from_core_slice(pids) {
      pid
        .send_system_message(actor_system.clone(), MessageHandle::new(MailboxMessage::ResumeMailbox))
        .await;
    }
  }
}
