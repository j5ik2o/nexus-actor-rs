use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream};

/// Embedded 環境向けの簡易 FailureEventHub 実装。
#[derive(Clone, Default)]
pub struct EmbeddedFailureEventHub {
  inner: Arc<Mutex<EmbeddedFailureEventHubState>>,
}

#[derive(Default)]
struct EmbeddedFailureEventHubState {
  next_id: u64,
  listeners: Vec<(u64, FailureEventListener)>,
}

pub struct EmbeddedFailureEventSubscription {
  inner: Arc<Mutex<EmbeddedFailureEventHubState>>,
  id: u64,
}

impl EmbeddedFailureEventHub {
  /// 新しい`EmbeddedFailureEventHub`を作成します。
  ///
  /// # Returns
  ///
  /// 新しいイベントハブインスタンス
  pub fn new() -> Self {
    Self {
      inner: Arc::new(Mutex::new(EmbeddedFailureEventHubState::default())),
    }
  }

  fn snapshot_listeners(&self) -> Vec<FailureEventListener> {
    let locked = self.inner.lock();
    locked.listeners.iter().map(|(_, listener)| listener.clone()).collect()
  }
}

impl FailureEventStream for EmbeddedFailureEventHub {
  type Subscription = EmbeddedFailureEventSubscription;

  fn listener(&self) -> FailureEventListener {
    let inner = self.clone();
    Arc::new(move |event: FailureEvent| {
      for listener in inner.snapshot_listeners().into_iter() {
        listener(event.clone());
      }
    })
  }

  fn subscribe(&self, listener: FailureEventListener) -> Self::Subscription {
    let id = {
      let mut state = self.inner.lock();
      let id = state.next_id;
      state.next_id = state.next_id.wrapping_add(1);
      state.listeners.push((id, listener));
      id
    };

    EmbeddedFailureEventSubscription {
      inner: self.inner.clone(),
      id,
    }
  }
}

impl Drop for EmbeddedFailureEventSubscription {
  fn drop(&mut self) {
    let mut state = self.inner.lock();
    if let Some(pos) = state.listeners.iter().position(|(entry_id, _)| *entry_id == self.id) {
      state.listeners.swap_remove(pos);
    }
  }
}
