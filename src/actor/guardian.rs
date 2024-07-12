use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::log::P_LOG;
use crate::actor::message::failure::Failure;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::system_message::SystemMessage;
use crate::actor::process::{Process, ProcessHandle};
use crate::actor::supervisor::supervisor_strategy::{Supervisor, SupervisorHandle, SupervisorStrategy};
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;
use crate::log::field::Field;

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
    match {
      let guardians = self.guardians.lock().await;
      guardians.get(&handle).cloned()
    } {
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
      .add_process(ph, &format!("guardian-{}", id));
    if !ok {
      P_LOG
        .error(
          "failed to register guardian process",
          vec![Field::stringer("pid", pid.clone())],
        )
        .await
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
          &self.guardians.actor_system,
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
  async fn get_children(&self) -> Vec<ExtendedPid> {
    panic!("guardian does not hold its children PIDs");
  }

  async fn escalate_failure(&self, _: ActorInnerError, _: MessageHandle) {
    panic!("guardian cannot escalate failure");
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      // Implement send_system_message for PID
      let restart_message = MessageHandle::new(SystemMessage::Restart);
      pid
        .send_system_message(self.guardians.actor_system.clone(), restart_message)
        .await;
    }
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      let restart_message = MessageHandle::new(SystemMessage::Stop);
      pid
        .send_system_message(self.guardians.actor_system.clone(), restart_message)
        .await;
    }
  }

  async fn resume_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      let restart_message = MessageHandle::new(MailboxMessage::ResumeMailbox);
      pid
        .send_system_message(self.guardians.actor_system.clone(), restart_message)
        .await;
    }
  }
}
