use super::ClusterFailureBridge;
use nexus_actor_core_rs::{ActorId, ActorPath, FailureEvent, FailureEventStream, FailureInfo};
use nexus_actor_std_rs::FailureEventHub;
use nexus_remote_core_rs::RemoteFailureNotifier;
use std::sync::{Arc, Mutex};

#[test]
fn cluster_failure_bridge_new_creates_instance() {
  let hub = FailureEventHub::new();
  let remote_notifier = RemoteFailureNotifier::new(hub.clone());
  let bridge = ClusterFailureBridge::new(hub, remote_notifier);

  let _listener = bridge.register();
}

#[test]
fn cluster_failure_bridge_register_returns_listener() {
  let hub = FailureEventHub::new();
  let remote_notifier = RemoteFailureNotifier::new(hub.clone());
  let bridge = ClusterFailureBridge::new(hub, remote_notifier);

  let _listener = bridge.register();
}

#[test]
fn cluster_failure_bridge_notifier_returns_reference() {
  let hub = FailureEventHub::new();
  let remote_notifier = RemoteFailureNotifier::new(hub.clone());
  let bridge = ClusterFailureBridge::new(hub, remote_notifier);

  let _notifier_ref = bridge.notifier();
}

#[test]
fn cluster_failure_bridge_fan_out_dispatches_root_escalation() {
  let hub = FailureEventHub::new();

  let hub_events = Arc::new(Mutex::new(Vec::new()));
  let hub_events_clone = Arc::clone(&hub_events);
  let _subscription = hub.subscribe(Arc::new(move |event: FailureEvent| {
    hub_events_clone.lock().unwrap().push(event);
  }));

  let remote_hub = FailureEventHub::new();
  let mut remote_notifier = RemoteFailureNotifier::new(remote_hub);

  let handler_called = Arc::new(Mutex::new(false));
  let handler_called_clone = Arc::clone(&handler_called);

  let handler = Arc::new(move |event: FailureEvent| {
    if matches!(event, FailureEvent::RootEscalated(_)) {
      *handler_called_clone.lock().unwrap() = true;
    }
  });
  remote_notifier.set_handler(handler);

  let bridge = ClusterFailureBridge::new(hub, remote_notifier);

  let info = FailureInfo::new(ActorId(1), ActorPath::new(), "test error".to_string());
  let event = FailureEvent::RootEscalated(info);

  bridge.fan_out(event.clone());

  assert!(*handler_called.lock().unwrap());

  let events = hub_events.lock().unwrap();
  assert_eq!(events.len(), 1);

  let FailureEvent::RootEscalated(received_info) = &events[0];
  let FailureEvent::RootEscalated(original_info) = &event;

  assert_eq!(received_info.actor, original_info.actor);
  assert_eq!(received_info.reason, original_info.reason);
}

#[test]
fn cluster_failure_bridge_fan_out_handles_hub_listener_call() {
  let hub = FailureEventHub::new();

  let hub_events = Arc::new(Mutex::new(Vec::new()));
  let hub_events_clone = Arc::clone(&hub_events);
  let _subscription = hub.subscribe(Arc::new(move |event: FailureEvent| {
    hub_events_clone.lock().unwrap().push(event);
  }));

  let remote_hub = FailureEventHub::new();
  let remote_notifier = RemoteFailureNotifier::new(remote_hub);

  let bridge = ClusterFailureBridge::new(hub, remote_notifier);

  let info = FailureInfo::new(ActorId(2), ActorPath::new(), "another error".to_string());
  let event = FailureEvent::RootEscalated(info);

  bridge.fan_out(event.clone());

  let events = hub_events.lock().unwrap();
  assert_eq!(events.len(), 1);
}
