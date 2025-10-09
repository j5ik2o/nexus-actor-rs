use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream};

/// FailureEventStream implementation for std.
#[derive(Clone, Default)]
pub struct FailureEventHub {
  inner: Arc<FailureEventHubInner>,
}

struct FailureEventHubInner {
  next_id: AtomicU64,
  listeners: Mutex<Vec<(u64, FailureEventListener)>>,
}

impl Default for FailureEventHubInner {
  fn default() -> Self {
    Self {
      next_id: AtomicU64::new(1),
      listeners: Mutex::new(Vec::new()),
    }
  }
}

impl FailureEventHub {
  /// Creates a new `FailureEventHub` instance.
  ///
  /// # Returns
  ///
  /// A new hub instance initialized with default state
  pub fn new() -> Self {
    Self::default()
  }

  fn notify_listeners(&self, event: FailureEvent) {
    let snapshot: Vec<FailureEventListener> = {
      let guard = self.inner.listeners.lock().unwrap();
      guard.iter().map(|(_, listener)| listener.clone()).collect()
    };
    for listener in snapshot.into_iter() {
      listener(event.clone());
    }
  }

  #[cfg(test)]
  pub(crate) fn listener_count(&self) -> usize {
    let guard = self.inner.listeners.lock().unwrap();
    guard.len()
  }
}

impl FailureEventStream for FailureEventHub {
  type Subscription = FailureEventSubscription;

  fn listener(&self) -> FailureEventListener {
    let inner = self.clone();
    FailureEventListener::new(move |event: FailureEvent| inner.notify_listeners(event))
  }

  fn subscribe(&self, listener: FailureEventListener) -> Self::Subscription {
    let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
    {
      let mut guard = self.inner.listeners.lock().unwrap();
      guard.push((id, listener));
    }

    FailureEventSubscription {
      inner: self.inner.clone(),
      id,
    }
  }
}

/// Subscription handle to FailureEventHub. Automatically unsubscribes on Drop.
pub struct FailureEventSubscription {
  inner: Arc<FailureEventHubInner>,
  id: u64,
}

impl Drop for FailureEventSubscription {
  fn drop(&mut self) {
    let mut guard = self.inner.listeners.lock().unwrap();
    if let Some(index) = guard.iter().position(|(entry_id, _)| *entry_id == self.id) {
      guard.swap_remove(index);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_actor_core_rs::{ActorId, ActorPath, FailureEvent, FailureEventListener, FailureInfo, FailureMetadata};
  use std::sync::Arc as StdArc;
  use std::sync::Mutex as StdMutex;

  #[test]
  fn hub_forwards_events_to_subscribers() {
    let hub = FailureEventHub::new();
    let storage: StdArc<StdMutex<Vec<FailureEvent>>> = StdArc::new(StdMutex::new(Vec::new()));
    let storage_clone = storage.clone();

    let _sub = hub.subscribe(FailureEventListener::new(move |event: FailureEvent| {
      storage_clone.lock().unwrap().push(event);
    }));

    let listener = hub.listener();
    let event = FailureEvent::RootEscalated(FailureInfo::new_with_metadata(
      ActorId(1),
      ActorPath::new(),
      "boom".into(),
      FailureMetadata::default(),
    ));
    listener(event.clone());

    let events = storage.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
      FailureEvent::RootEscalated(info) => assert_eq!(info.reason, "boom"),
    }
  }

  #[test]
  fn subscription_drop_removes_listener() {
    let hub = FailureEventHub::new();
    assert_eq!(hub.listener_count(), 0);
    let subscription = hub.subscribe(FailureEventListener::new(|_| {}));
    assert_eq!(hub.listener_count(), 1);
    drop(subscription);
    assert_eq!(hub.listener_count(), 0);
  }
}
