use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::Stop;
use crate::actor::actor_system::ActorSystem;
use crate::actor::log::P_LOG;
use crate::actor::message::{Message, MessageHandle};
use crate::actor::messages::{Failure, MailboxMessage, Restart, SystemMessage};
use crate::actor::pid::ExtendedPid;
use crate::actor::process::{Process, ProcessHandle};
use crate::actor::supervisor_strategy::{Supervisor, SupervisorHandle, SupervisorStrategy, SupervisorStrategyHandle};
use crate::actor::ReasonHandle;
use crate::log::field::Field;

#[derive(Debug, Clone)]
pub struct Guardians {
  actor_system: ActorSystem,
  guardians: Arc<Mutex<HashMap<SupervisorStrategyHandle, GuardianProcess>>>,
}

impl Guardians {
  pub fn new(actor_system: ActorSystem) -> Self {
    Guardians {
      actor_system,
      guardians: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub async fn get_guardian_pid(&self, s: SupervisorStrategyHandle) -> ExtendedPid {
    let handle = s.clone();
    match {
      let mut guardians = self.guardians.lock().await;
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
        let guardian = GuardianProcess::new_guardian(Arc::new(self.clone()), s.clone()).await;
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
  guardians: Arc<Guardians>,
  pid: Arc<Option<ExtendedPid>>,
  strategy: SupervisorStrategyHandle,
}

impl GuardianProcess {
  async fn new_guardian(guardians: Arc<Guardians>, s: SupervisorStrategyHandle) -> GuardianProcess {
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
      .add(ph, &format!("guardian-{}", id));
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
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message: MessageHandle) {
    panic!("guardian actor cannot receive any user messages");
  }

  async fn send_system_message(&self, _pid: &ExtendedPid, message: MessageHandle) {
    if let Some(failure) = message.as_any().downcast_ref::<Failure>() {
      self
        .strategy
        .handle_failure(
          &self.guardians.actor_system,
          SupervisorHandle::new(self.clone()),
          failure.who.clone(),
          failure.restart_stats.clone(),
          failure.reason.clone(),
          failure.message.clone(),
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

  async fn escalate_failure(&mut self, _reason: ReasonHandle, _message: MessageHandle) {
    panic!("guardian cannot escalate failure");
  }

  async fn restart_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      // Implement send_system_message for PID
      let restart_message = MessageHandle::new(SystemMessage::Restart(Restart {}));
      pid
        .send_system_message(self.guardians.actor_system.clone(), restart_message)
        .await;
    }
  }

  async fn stop_children(&self, pids: &[ExtendedPid]) {
    for pid in pids {
      let restart_message = MessageHandle::new(SystemMessage::Stop(Stop {}));
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
