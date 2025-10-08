use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::failure::FailureEvent;
use crate::runtime::supervision::FailureEventListener;

/// ルート EscalationSink からの FailureEvent を複数購読者へ配信するヘルパ。
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
  pub fn new() -> Self {
    Self::default()
  }

  /// EscalationSink へ渡す FailureEventListener を生成する。
  pub fn listener(&self) -> FailureEventListener {
    let inner = self.inner.clone();
    Arc::new(move |event: FailureEvent| {
      let snapshot: Vec<FailureEventListener> = {
        let guard = inner.listeners.lock().unwrap();
        guard.iter().map(|(_, l)| Arc::clone(l)).collect()
      };
      for listener in snapshot.into_iter() {
        listener(event.clone());
      }
    })
  }

  /// 新しい購読者を登録し、ドロップ時に自動解除される Subscription を返す。
  pub fn subscribe(&self, listener: FailureEventListener) -> FailureEventSubscription {
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

  pub fn listener_count(&self) -> usize {
    let guard = self.inner.listeners.lock().unwrap();
    guard.len()
  }
}

/// FailureEventHub への購読ハンドル。Drop 時に自動解除される。
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
  use crate::failure::{FailureInfo, FailureMetadata};
  use crate::ActorId;
  use std::sync::Mutex as StdMutex;

  #[test]
  fn hub_forwards_events_to_subscribers() {
    let hub = FailureEventHub::new();
    let storage = Arc::new(StdMutex::new(Vec::new()));
    let storage_clone = storage.clone();

    let _sub = hub.subscribe(Arc::new(move |event: FailureEvent| {
      storage_clone.lock().unwrap().push(event);
    }));

    let listener = hub.listener();
    let event = FailureEvent::RootEscalated(FailureInfo::new_with_metadata(
      ActorId(1),
      crate::ActorPath::new(),
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
}
