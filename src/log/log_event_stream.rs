use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;

use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::log::log::Level;
use crate::log::log_event::LogEvent;
use crate::log::subscription::Subscription;

pub static LOG_EVENT_STREAM: Lazy<Arc<LogEventStream>> = Lazy::new(|| Arc::new(LogEventStream::new()));

pub fn get_global_log_event_stream() -> Arc<LogEventStream> {
  LOG_EVENT_STREAM.clone()
}

pub struct LogEventStream {
  pub(crate) subscriptions: Arc<RwLock<Vec<Arc<Subscription>>>>,
}

impl LogEventStream {
  pub fn new() -> Self {
    Self {
      subscriptions: Arc::new(RwLock::new(Vec::new())),
    }
  }

  pub async fn subscribe<F, Fut>(self: &Arc<Self>, f: F) -> Arc<Subscription>
  where
    F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
    Fut: futures::Future<Output = ()> + Send + 'static, {
    let mut subscriptions = self.subscriptions.write().await;
    let sub = Arc::new(Subscription {
      event_stream: Arc::downgrade(&self.clone()),
      index: Arc::new(AtomicUsize::new(subscriptions.len())),
      func: EventHandler::new(f),
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

  pub async fn publish(&self, evt: LogEvent) {
    let subscriptions = self.subscriptions.read().await;
    for sub in subscriptions.iter() {
      if evt.level >= Level::try_from(sub.min_level.load(Ordering::Relaxed)).unwrap() {
        sub.func.clone().run(evt.clone()).await;
      }
    }
  }

  pub async fn clear(&self) {
    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.clear();
  }
}

#[derive(Clone)]
pub struct EventHandler(Arc<dyn Fn(LogEvent) -> BoxFuture<'static, ()> + Send + Sync>);

impl EventHandler {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
    Fut: futures::Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |evt| Box::pin(f(evt))))
  }

  pub async fn run(self, event: LogEvent) {
    self.0(event).await
  }
}

impl Debug for EventHandler {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "EventHandler")
  }
}

impl PartialEq for EventHandler {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for EventHandler {}

impl std::hash::Hash for EventHandler {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(LogEvent) -> BoxFuture<'static, ()>).hash(state);
  }
}

pub async fn subscribe_stream<F, Fut>(event_stream: &Arc<LogEventStream>, f: F) -> Arc<Subscription>
where
  F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
  Fut: futures::Future<Output = ()> + Send + 'static, {
  event_stream.subscribe(f).await
}

pub async fn unsubscribe_stream(sub: &Arc<Subscription>) {
  if let Some(event_stream) = sub.event_stream.upgrade() {
    event_stream.unsubscribe(sub).await;
  }
}

pub async fn publish_to_stream(event_stream: &Arc<LogEventStream>, evt: LogEvent) {
  event_stream.publish(evt).await;
}

pub async fn reset_event_stream() {
  LOG_EVENT_STREAM.clear().await;
}
