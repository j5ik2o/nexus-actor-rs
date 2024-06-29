use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::log::event::Event;
use crate::log::log::Level;

pub static EVENT_STREAM: Lazy<Arc<EventStream>> = Lazy::new(|| Arc::new(EventStream::new()));

pub fn get_global_event_stream() -> Arc<EventStream> {
  EVENT_STREAM.clone()
}

pub struct EventStream {
  pub(crate) subscriptions: Arc<RwLock<Vec<Arc<Subscription>>>>,
}

impl EventStream {
  pub fn new() -> Self {
    Self {
      subscriptions: Arc::new(RwLock::new(Vec::new())),
    }
  }

  pub async fn subscribe<F>(self: &Arc<Self>, f: F) -> Arc<Subscription>
  where
    F: Fn(Event) + Send + Sync + 'static, {
    let mut subscriptions = self.subscriptions.write().await;
    let sub = Arc::new(Subscription {
      event_stream: Arc::downgrade(&self.clone()),
      index: Arc::new(AtomicUsize::new(subscriptions.len())),
      func: EventFunc::new(f),
      min_level: Arc::new(AtomicI32::new(Level::Min as i32)),
    });
    subscriptions.push(Arc::clone(&sub));
    sub
  }

  pub async fn unsubscribe(&self, sub: &Arc<Subscription>) {
    let mut subscriptions = self.subscriptions.write().await;
    if let Some(index) = subscriptions.iter().position(|s| Arc::ptr_eq(s, sub)) {
      let last = subscriptions.len() - 1;
      subscriptions.swap(index, last);
      if let Some(swapped) = subscriptions.get(index) {
        swapped.index.store(index, Ordering::Relaxed);
      }
      subscriptions.pop();
    }
  }

  pub async fn publish(&self, evt: Event) {
    let subscriptions = self.subscriptions.read().await;
    for sub in subscriptions.iter() {
      if evt.level >= Level::try_from(sub.min_level.load(Ordering::Relaxed)).unwrap() {
        sub.func.clone().run(evt.clone());
      }
    }
  }

  pub async fn clear(&self) {
    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.clear();
  }
}

#[derive(Clone)]
pub struct EventFunc(Arc<dyn Fn(Event) + Send + Sync>);

impl Debug for EventFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "EventFunc")
  }
}

impl PartialEq for EventFunc {
  fn eq(&self, _other: &Self) -> bool {
    true
  }
}

impl Eq for EventFunc {}

impl std::hash::Hash for EventFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(Event)).hash(state);
  }
}

impl EventFunc {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(Event) + Send + Sync + 'static, {
    EventFunc(Arc::new(f))
  }

  pub fn run(self, event: Event) {
    self.0(event)
  }
}

#[derive(Debug, Clone)]
pub struct Subscription {
  event_stream: Weak<EventStream>,
  index: Arc<AtomicUsize>,
  func: EventFunc,
  min_level: Arc<AtomicI32>,
}

impl Subscription {
  pub fn with_min_level(self: &Arc<Self>, level: Level) -> Arc<Self> {
    self.min_level.store(level as i32, Ordering::Relaxed);
    Arc::clone(self)
  }
}

pub async fn subscribe_stream<F>(event_stream: &Arc<EventStream>, f: F) -> Arc<Subscription>
where
  F: Fn(Event) + Send + Sync + 'static, {
  event_stream.subscribe(f).await
}

pub async fn unsubscribe_stream(sub: &Arc<Subscription>) {
  if let Some(event_stream) = sub.event_stream.upgrade() {
    event_stream.unsubscribe(sub).await;
  }
}

pub async fn publish_to_stream(event_stream: &Arc<EventStream>, evt: Event) {
  event_stream.publish(evt).await;
}

pub async fn reset_event_stream(event_stream: &Arc<EventStream>) {
  event_stream.clear().await;
}
