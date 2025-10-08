use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::context::SenderPart;
use crate::actor::core::ExtendedPid;
use crate::actor::message::unwrap_envelope;
use crate::actor::message::IgnoreDeadLetterLogging;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::message::SystemMessage;
use crate::actor::message::TerminateReason;
use crate::actor::metrics::metrics_impl::{MetricsRuntime, MetricsSink};
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::DeadLetterResponse;

use crate::actor::dispatch::throttler::{Throttle, Valve};
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use nexus_message_derive_rs::Message;
use opentelemetry::KeyValue;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DeadLetterProcess {
  actor_system: WeakActorSystem,
  metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  metrics_sink: Arc<ArcSwapOption<MetricsSink>>,
}

impl DeadLetterProcess {
  const METRICS_ACTOR_NAME: &'static str = "dead_letter_process";

  pub async fn new(actor_system: ActorSystem) -> Self {
    let metrics_runtime = actor_system.metrics_runtime_slot();
    let metrics_sink = Arc::new(ArcSwapOption::from(None::<Arc<MetricsSink>>));

    let myself = Self {
      actor_system: actor_system.downgrade(),
      metrics_runtime,
      metrics_sink,
    };
    let config = myself.actor_system().get_config();
    let dead_letter_throttle_count = config.dead_letter_throttle_count;
    let dead_letter_throttle_interval = config.dead_letter_throttle_interval;
    let func =
      move |i: usize| async move { tracing::info!("DeadLetterProcess: Throttling dead letters, count: {}", i) };
    let scheduler = myself.actor_system().core_runtime().scheduler();
    let throttle = Throttle::new(
      scheduler,
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

            if cloned_self.actor_system().get_config().developer_supervision_logging && dead_letter.sender.is_some() {
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

            if let Some(sink) = cloned_self.metrics_sink() {
              let mut labels = vec![
                KeyValue::new("deadletter.phase", "event_stream_notify"),
                KeyValue::new("deadletter.event", "dead_letter"),
                KeyValue::new("deadletter.message_type", dead_letter.message_handle.get_type_name()),
              ];
              if let Some(target) = &dead_letter.pid {
                labels.push(KeyValue::new("deadletter.target_pid", target.id().to_string()));
              }
              if let Some(sender) = &dead_letter.sender {
                labels.push(KeyValue::new("deadletter.sender_pid", sender.id().to_string()));
              }
              sink.increment_dead_letter_with_labels(&labels);
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
              let watcher_core = watch.watcher().clone();
              let e_pid = ExtendedPid::from_core(watcher_core.clone());
              e_pid
                .send_system_message(
                  actor_system,
                  MessageHandle::new(SystemMessage::terminate(
                    Some(watcher_core.clone()),
                    TerminateReason::NotFound,
                  )),
                )
                .await;

              if let Some(sink) = cloned_self.metrics_sink() {
                let mut labels = vec![
                  KeyValue::new("deadletter.phase", "event_stream_watch"),
                  KeyValue::new("deadletter.event", "watch_termination"),
                  KeyValue::new("deadletter.message_type", dle.message_handle.get_type_name()),
                ];
                labels.push(KeyValue::new("deadletter.watcher_pid", e_pid.id().to_string()));
                if let Some(target) = &dle.pid {
                  labels.push(KeyValue::new("deadletter.target_pid", target.id().to_string()));
                }
                sink.increment_dead_letter_with_labels(&labels);
              }
            }
          }
        }
      })
      .await;

    myself
  }

  fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    if let Some(existing) = self.metrics_sink.load_full() {
      return Some(existing);
    }
    let factory = self.metrics_runtime.load_full()?;
    let sink = Arc::new(runtime.sink_for_actor(Some(Self::METRICS_ACTOR_NAME)));
    self.metrics_sink.store(Some(sink.clone()));
    Some(sink)
  }
}

#[async_trait]
impl Process for DeadLetterProcess {
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message_handle: MessageHandle) {
    tracing::debug!("DeadLetterProcess: send_user_message: msg = {:?}", message_handle);
    if let Some(sink) = self.metrics_sink() {
      let mut labels = vec![KeyValue::new("deadletter.phase", "publish")];
      labels.push(KeyValue::new("deadletter.message_type", message_handle.get_type_name()));
      if let Some(pid) = pid {
        labels.push(KeyValue::new("deadletter.target_pid", pid.id().to_string()));
      }
      sink.increment_dead_letter_with_labels(&labels);
    }

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
