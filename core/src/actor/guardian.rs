use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::ErrorReason;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::dispatch::MailboxMessage;
use crate::actor::message::Failure;
use crate::actor::message::MessageHandle;
use crate::actor::message::SystemMessage;
use crate::actor::process::{Process, ProcessHandle};
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::actor::supervisor::{Supervisor, SupervisorHandle, SupervisorStrategy};

#[derive(Debug, Clone)]
pub struct GuardiansValue {
  actor_system: ActorSystem,
  guardians: Arc<Mutex<HashMap<SupervisorStrategyHandle, GuardianProcess>>>,
}

impl GuardiansValue {
  pub fn new(actor_system: ActorSystem) -> Self {
    GuardiansValue {
      actor_system,
      guardians: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub async fn get_guardian_pid(&self, s: SupervisorStrategyHandle) -> ExtendedPid {
    let handle = s.clone();
    let res = {
      let guardians = self.guardians.lock().await;
      guardians.get(&handle).cloned()
    };
    match res {
      Some(guardian) => {
        let pid = guardian.pid.clone();
        let op = match &*pid {
          Some(p) => p.clone(),
          None => {
            panic!("Guardian PID is not initialized");
          }
        };
        op
      }
      None => {
        let guardian = GuardianProcess::new(Arc::new(self.clone()), s.clone()).await;
        {
          let mut guardians = self.guardians.lock().await;
          guardians.insert(s.clone(), guardian.clone());
        }
        let pid = guardian.pid.clone();
        let op = match &*pid {
          Some(p) => p.clone(),
          None => {
            panic!("Guardian PID is not initialized");
          }
        };
        op
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
    let id = guardians.actor_system.get_process_registry().await.next_id();
    let ph = ProcessHandle::new(guardian.clone());
    let (pid, ok) = guardians
      .actor_system
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
      self
        .strategy
        .handle_child_failure(
          self.guardians.actor_system.clone(),
          SupervisorHandle::new(self.clone()),
          failure.who.clone(),
          failure.restart_stats.clone(),
          failure.reason.clone(),
          failure.message_handle.clone(),
        )
        .await;
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

  async fn get_children(&self) -> Vec<ExtendedPid> {
    panic!("guardian does not hold its children PIDs");
  }

  async fn escalate_failure(&self, _: ErrorReason, _: MessageHandle) {
    panic!("guardian cannot escalate failure");
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      // Implement send_system_message for PID
      pid
        .send_system_message(
          self.guardians.actor_system.clone(),
          MessageHandle::new(SystemMessage::Restart),
        )
        .await;
    }
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      pid
        .send_system_message(
          self.guardians.actor_system.clone(),
          MessageHandle::new(SystemMessage::Stop),
        )
        .await;
    }
  }

  async fn resume_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      pid
        .send_system_message(
          self.guardians.actor_system.clone(),
          MessageHandle::new(MailboxMessage::ResumeMailbox),
        )
        .await;
    }
  }
}
