#[cfg(test)]
mod tests {
  use crate::{placeholder_metadata, RemoteFailureNotifier};
  use nexus_actor_core_rs::{ActorId, ActorPath, FailureEvent, FailureEventHub, FailureInfo};
  use std::sync::{Arc, Mutex};

  #[test]
  fn remote_failure_notifier_new_creates_instance() {
    let hub = FailureEventHub::new();
    let notifier = RemoteFailureNotifier::new(hub);

    assert!(notifier.handler().is_none());
  }

  #[test]
  fn remote_failure_notifier_listener_returns_hub_listener() {
    let hub = FailureEventHub::new();
    let notifier = RemoteFailureNotifier::new(hub);

    let _listener = notifier.listener();
    // listener is a function, just verify it can be obtained
  }

  #[test]
  fn remote_failure_notifier_hub_returns_reference() {
    let hub = FailureEventHub::new();
    let notifier = RemoteFailureNotifier::new(hub);

    let _hub_ref = notifier.hub();
    // Just verify we can get a reference
  }

  #[test]
  fn remote_failure_notifier_set_handler_stores_handler() {
    let hub = FailureEventHub::new();
    let mut notifier = RemoteFailureNotifier::new(hub);

    assert!(notifier.handler().is_none());

    let handler = Arc::new(|_event: FailureEvent| {});
    notifier.set_handler(handler);

    assert!(notifier.handler().is_some());
  }

  #[test]
  fn remote_failure_notifier_dispatch_calls_handler() {
    let hub = FailureEventHub::new();
    let mut notifier = RemoteFailureNotifier::new(hub);

    let called = Arc::new(Mutex::new(false));
    let called_clone = called.clone();

    let handler = Arc::new(move |event: FailureEvent| {
      if let FailureEvent::RootEscalated(_) = event {
        *called_clone.lock().unwrap() = true;
      }
    });
    notifier.set_handler(handler);

    let info = FailureInfo::new(ActorId(1), ActorPath::new(), "test error".to_string());
    notifier.dispatch(info);

    assert!(*called.lock().unwrap());
  }

  #[test]
  fn remote_failure_notifier_dispatch_without_handler_does_nothing() {
    let hub = FailureEventHub::new();
    let notifier = RemoteFailureNotifier::new(hub);

    let info = FailureInfo::new(ActorId(1), ActorPath::new(), "test error".to_string());
    notifier.dispatch(info);
    // Should not panic
  }

  #[test]
  fn remote_failure_notifier_emit_calls_hub_and_handler() {
    let hub = FailureEventHub::new();

    let hub_events = Arc::new(Mutex::new(Vec::new()));
    let hub_events_clone = hub_events.clone();
    let _subscription = hub.subscribe(Arc::new(move |event: FailureEvent| {
      hub_events_clone.lock().unwrap().push(event);
    }));

    let mut notifier = RemoteFailureNotifier::new(hub);

    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    let handler = Arc::new(move |event: FailureEvent| {
      if let FailureEvent::RootEscalated(_) = event {
        *handler_called_clone.lock().unwrap() = true;
      }
    });
    notifier.set_handler(handler);

    let info = FailureInfo::new(ActorId(1), ActorPath::new(), "test error".to_string());
    notifier.emit(info.clone());

    // Check handler was called
    assert!(*handler_called.lock().unwrap());

    // Check hub received the event
    let events = hub_events.lock().unwrap();
    assert_eq!(events.len(), 1);
    if let FailureEvent::RootEscalated(received_info) = &events[0] {
      assert_eq!(received_info.actor, info.actor);
      assert_eq!(received_info.reason, info.reason);
    } else {
      panic!("Expected RootEscalated event");
    }
  }

  #[test]
  fn placeholder_metadata_creates_metadata_with_endpoint() {
    let endpoint = "localhost:8080";
    let metadata = placeholder_metadata(endpoint);

    assert_eq!(metadata.endpoint, Some(endpoint.to_string()));
    assert!(metadata.component.is_none());
    assert!(metadata.transport.is_none());
    assert!(metadata.tags.is_empty());
  }
}
