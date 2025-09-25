use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::context::SenderPart;
use crate::actor::core::ExtendedPid;
use crate::actor::message::unwrap_envelope;
use crate::actor::message::IgnoreDeadLetterLogging;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::message::SystemMessage;
use crate::actor::message::TerminateReason;
use crate::actor::metrics::metrics_impl::{Metrics, EXTENSION_ID};
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::{DeadLetterResponse, Terminated};

use crate::actor::dispatch::throttler::{Throttle, Valve};
use crate::metrics::ActorMetrics;
use async_trait::async_trait;
use nexus_actor_message_derive_rs::Message;

#[derive(Debug, Clone)]
pub struct DeadLetterProcess {
  actor_system: WeakActorSystem,
}

impl DeadLetterProcess {
  pub async fn new(actor_system: ActorSystem) -> Self {
    let myself = Self {
      actor_system: actor_system.downgrade(),
    };
    let dead_letter_throttle_count = myself.actor_system().get_config().await.dead_letter_throttle_count;
    let dead_letter_throttle_interval = myself.actor_system().get_config().await.dead_letter_throttle_interval;
    let func =
      move |i: usize| async move { tracing::info!("DeadLetterProcess: Throttling dead letters, count: {}", i) };
    let dispatcher = myself.actor_system().get_config().await.system_dispatcher.clone();
    let throttle = Throttle::new(
      dispatcher,
      dead_letter_throttle_count,
      dead_letter_throttle_interval,
      func,
    )
    .await;

    let cloned_self = myself.clone();
    let actor_system = myself.actor_system();
    actor_system
      .get_process_registry()
      .await
      .add_process(ProcessHandle::new(myself.clone()), "deadletter")
      .await;
    actor_system
      .get_event_stream()
      .await
      .subscribe(move |msg| {
        let cloned_msg = msg.clone();
        let cloned_self = cloned_self.clone();
        let cloned_throttle = throttle.clone();
        async move {
          if let Some(dead_letter) = cloned_msg.to_typed::<DeadLetterEvent>() {
            if let Some(sender) = &dead_letter.sender {
              cloned_self
                .actor_system()
                .get_root_context()
                .await
                .send(sender.clone(), MessageHandle::new(DeadLetterResponse { target: None }))
                .await
            }

            if cloned_self
              .actor_system()
              .get_config()
              .await
              .developer_supervision_logging
              && dead_letter.sender.is_some()
            {
              return;
            }

            if let Some(is_ignore_dead_letter) = dead_letter.message_handle.to_typed::<IgnoreDeadLetterLogging>() {
              if cloned_throttle.should_throttle() == Valve::Open {
                tracing::debug!(
                  "DeadLetterProcess: Message from {} to {} was not delivered, message: {:?}",
                  dead_letter.sender.as_ref().unwrap(),
                  dead_letter
                    .pid
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or("None".to_string()),
                  is_ignore_dead_letter
                );
              }
            }
          }
        }
      })
      .await;

    let cloned_self = myself.clone();
    actor_system
      .get_event_stream()
      .await
      .subscribe(move |msg| {
        let cloned_msg = msg.clone();
        let cloned_self = cloned_self.clone();
        async move {
          if let Some(dle) = cloned_msg.to_typed::<DeadLetterEvent>() {
            if let Some(SystemMessage::Watch(watch)) = dle.message_handle.to_typed::<SystemMessage>() {
              let actor_system = cloned_self.actor_system();
              let pid = watch.watcher.clone().unwrap();
              let e_pid = ExtendedPid::new(pid.clone());
              e_pid
                .send_system_message(
                  actor_system,
                  MessageHandle::new(SystemMessage::Terminate(Terminated {
                    who: Some(pid),
                    why: TerminateReason::NotFound as i32,
                  })),
                )
                .await;
            }
          }
        }
      })
      .await;

    myself
  }

  async fn metrics_foreach<F, Fut>(&self, f: F)
  where
    F: Fn(&ActorMetrics, &Metrics) -> Fut,
    Fut: std::future::Future<Output = ()>,
  {
    let actor_system = self.actor_system();
    if actor_system.get_config().await.is_metrics_enabled() {
      if let Some(extension_arc) = actor_system.get_extensions().await.get(*EXTENSION_ID).await {
        let mut extension = extension_arc.lock().await;
        if let Some(m) = extension.as_any_mut().downcast_mut::<Metrics>() {
          m.foreach(f).await;
        }
      }
    }
  }
}

#[async_trait]
impl Process for DeadLetterProcess {
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message_handle: MessageHandle) {
    tracing::debug!("DeadLetterProcess: send_user_message: msg = {:?}", message_handle);
    self
      .metrics_foreach(|am, _| {
        let am = am.clone();
        async move {
          am.increment_dead_letter_count();
        }
      })
      .await;

    let (_, msg, sender) = unwrap_envelope(message_handle.clone());
    self
      .actor_system()
      .get_event_stream()
      .await
      .publish(MessageHandle::new(DeadLetterEvent {
        pid: pid.cloned(),
        message_handle: msg,
        sender,
      }))
      .await;
    tracing::debug!("DeadLetterProcess: send_user_message: msg = {:?}", message_handle);
  }

  async fn send_system_message(&self, pid: &ExtendedPid, message_handle: MessageHandle) {
    self
      .actor_system()
      .get_event_stream()
      .await
      .publish(MessageHandle::new(DeadLetterEvent {
        pid: Some(pid.clone()),
        message_handle: message_handle.clone(),
        sender: None,
      }))
      .await;
    tracing::debug!("DeadLetterProcess: send_system_message: msg = {:?}", message_handle);
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self
      .send_system_message(pid, MessageHandle::new(SystemMessage::Stop))
      .await
  }

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn std::any::Any {
    self
  }
}

impl DeadLetterProcess {
  fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before DeadLetterProcess")
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct DeadLetterEvent {
  pub pid: Option<ExtendedPid>,
  pub message_handle: MessageHandle,
  pub sender: Option<ExtendedPid>,
}
