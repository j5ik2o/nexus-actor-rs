use std::any::Any;

use async_trait::async_trait;

use crate::actor::actor::{DeadLetterResponse, Stop, Terminated, TerminatedReason, Watch};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::SenderPart;
use crate::actor::log::P_LOG;
use crate::actor::message::{Message, MessageHandle};
use crate::actor::message_envelope::unwrap_envelope;
use crate::actor::messages::{IgnoreDeadLetterLogging, SystemMessage};
use crate::actor::pid::ExtendedPid;
use crate::actor::process::{Process, ProcessHandle};
use crate::actor::throttler::{Throttle, ThrottleCallbackFunc, Valve};
use crate::event_stream::HandlerFunc;

#[derive(Debug, Clone)]
pub struct DeadLetterProcess {
  actor_system: ActorSystem,
}

impl DeadLetterProcess {
  pub async fn new(actor_system: ActorSystem) -> Self {
    let myself = Self { actor_system };
    let dead_letter_throttle_count = myself
      .actor_system
      .get_config()
      .await
      .dead_letter_throttle_count
      .clone();
    let dead_letter_throttle_interval = myself
      .actor_system
      .get_config()
      .await
      .dead_letter_throttle_interval
      .clone();
    let func = ThrottleCallbackFunc::new(move |i: usize| async move {
      P_LOG
        .info(
          &format!("DeadLetterProcess: Throttling dead letters, count: {}", i),
          vec![],
        )
        .await;
    });
    let throttle = Throttle::new(dead_letter_throttle_count, dead_letter_throttle_interval, func).await;
    println!("dead_letter_process: new: throttle: {:?}", throttle);

    let cloned_self = myself.clone();
    myself
      .actor_system
      .get_process_registry()
      .await
      .add_process(ProcessHandle::new(myself.clone()), "deadletter");
    myself
      .actor_system
      .get_event_stream()
      .await
      .subscribe(HandlerFunc::new(move |msg| {
        let cloned_msg = msg.clone();
        let cloned_self = cloned_self.clone();
        let cloned_throttle = throttle.clone();
        async move {
          if let Some(dead_letter) = cloned_msg.as_any().downcast_ref::<DeadLetterEvent>() {
            if let Some(sender) = &dead_letter.sender {
              cloned_self
                .actor_system
                .get_root_context()
                .await
                .send(sender.clone(), MessageHandle::new(DeadLetterResponse { target: None }))
                .await
            }

            if cloned_self
              .actor_system
              .get_config()
              .await
              .developer_supervision_logging
              && dead_letter.sender.is_some()
            {
              return;
            }

            if let Some(is_ignore_dead_letter) = dead_letter.message.as_any().downcast_ref::<IgnoreDeadLetterLogging>()
            {
              if cloned_throttle.should_throttle() == Valve::Open {
                P_LOG
                  .debug(
                    &format!(
                      "DeadLetterProcess: Message from {} to {} was not delivered, message: {:?}",
                      dead_letter.sender.as_ref().unwrap(),
                      dead_letter.pid,
                      is_ignore_dead_letter,
                    ),
                    vec![],
                  )
                  .await
              }
            }
          }
        }
      }))
      .await;

    let cloned_self = myself.clone();
    myself
      .actor_system
      .get_event_stream()
      .await
      .subscribe(HandlerFunc::new(move |msg| {
        let cloned_msg = msg.clone();
        let cloned_self = cloned_self.clone();
        async move {
          if let Some(dle) = cloned_msg.as_any().downcast_ref::<DeadLetterEvent>() {
            if let Some(m) = dle.message.as_any().downcast_ref::<Watch>() {
              let actor_system = cloned_self.actor_system.clone();
              let pid = m.watcher.clone().unwrap();
              let e_pid = ExtendedPid::new(pid.clone(), actor_system.clone());
              e_pid
                .send_system_message(
                  actor_system,
                  MessageHandle::new(Terminated {
                    who: Some(pid),
                    why: TerminatedReason::NotFound as i32,
                  }),
                )
                .await;
            }
          }
        }
      }))
      .await;

    myself
  }
}

#[async_trait]
impl Process for DeadLetterProcess {
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message: MessageHandle) {
    // TODO: Metrics

    let (_, msg, sender) = unwrap_envelope(message);
    self
      .actor_system
      .get_event_stream()
      .await
      .publish(MessageHandle::new(DeadLetterEvent {
        pid: pid.unwrap().clone(),
        message: msg,
        sender,
      }))
      .await;
  }

  async fn send_system_message(&self, pid: &ExtendedPid, message: MessageHandle) {
    self
      .actor_system
      .get_event_stream()
      .await
      .publish(MessageHandle::new(DeadLetterEvent {
        pid: pid.clone(),
        message,
        sender: None,
      }))
      .await;
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self
      .send_system_message(pid, MessageHandle::new(SystemMessage::Stop(Stop {})))
      .await
  }

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn std::any::Any {
    self
  }
}

#[derive(Debug, Clone)]
pub struct DeadLetterEvent {
  pub pid: ExtendedPid,
  pub message: MessageHandle,
  pub sender: Option<ExtendedPid>,
}

impl Message for DeadLetterEvent {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
