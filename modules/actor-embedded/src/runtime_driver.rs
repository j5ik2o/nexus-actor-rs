use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use nexus_actor_core_rs::{ActorRef, ActorSystem, MessageEnvelope, PriorityEnvelope, Props, RuntimeComponents};
use nexus_utils_embedded_rs::{Element, QueueError};

use crate::{ImmediateSpawner, ImmediateTimer, LocalMailboxRuntime};

use nexus_actor_core_rs::FailureEvent;
use nexus_actor_core_rs::{FailureEventListener, FailureEventStream};

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

/// Embedded ランタイムのランタイムドライバ。
pub struct EmbeddedActorRuntime<U>
where
  U: Element, {
  system: ActorSystem<U, LocalMailboxRuntime>,
  spawner: ImmediateSpawner,
  timer: ImmediateTimer,
  events: EmbeddedFailureEventHub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
  Busy,
  Idle,
}

impl<U> EmbeddedActorRuntime<U>
where
  U: Element,
{
  pub fn new() -> Self {
    let components = RuntimeComponents::new(
      LocalMailboxRuntime::default(),
      ImmediateSpawner,
      ImmediateTimer,
      EmbeddedFailureEventHub::new(),
    );

    let (system, handles) = ActorSystem::from_runtime_components(components);

    Self {
      system,
      spawner: handles.spawner,
      timer: handles.timer,
      events: handles.event_stream,
    }
  }

  pub fn spawn_actor(
    &mut self,
    props: Props<U, LocalMailboxRuntime>,
  ) -> Result<ActorRef<U, LocalMailboxRuntime>, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let mut root = self.system.root_context();
    root.spawn(props)
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.system.dispatch_next().await
  }

  pub fn pump(&mut self) -> DriverState {
    DriverState::Idle
  }

  pub fn event_stream(&self) -> &EmbeddedFailureEventHub {
    &self.events
  }

  pub fn spawner(&self) -> &ImmediateSpawner {
    &self.spawner
  }

  pub fn timer(&self) -> &ImmediateTimer {
    &self.timer
  }
}
