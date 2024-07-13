use std::fmt::Debug;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;

use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::log::log::Level;
use crate::log::log_event::LogEvent;
use crate::log::log_event_handler::LogEventHandler;
use crate::log::log_subscription::LogSubscription;

pub static LOG_EVENT_STREAM: Lazy<Arc<LogEventStream>> = Lazy::new(|| LogEventStream::new());

pub fn get_global_log_event_stream() -> Arc<LogEventStream> {
  LOG_EVENT_STREAM.clone()
}

pub struct LogEventStream {
  pub(crate) subscriptions: Arc<RwLock<Vec<Arc<LogSubscription>>>>,
}

impl LogEventStream {
  pub fn new() -> Arc<Self> {
    Arc::new(Self {
      subscriptions: Arc::new(RwLock::new(Vec::new())),
    })
  }

  pub async fn subscribe<F, Fut>(self: &Arc<Self>, f: F) -> Arc<LogSubscription>
  where
    F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
    Fut: futures::Future<Output = ()> + Send + 'static, {
    let mut subscriptions = self.subscriptions.write().await;
    let sub = Arc::new(LogSubscription {
      event_stream: Arc::downgrade(&self.clone()),
      index: Arc::new(AtomicUsize::new(subscriptions.len())),
      func: LogEventHandler::new(f),
      min_level: Arc::new(AtomicI32::new(Level::Min as i32)),
    });
    subscriptions.push(Arc::clone(&sub));
    sub
  }

  pub async fn unsubscribe(&self, sub: &Arc<LogSubscription>) {
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

pub async fn subscribe_stream<F, Fut>(event_stream: &Arc<LogEventStream>, f: F) -> Arc<LogSubscription>
where
  F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
  Fut: futures::Future<Output = ()> + Send + 'static, {
  event_stream.subscribe(f).await
}

pub async fn unsubscribe_stream(sub: &Arc<LogSubscription>) {
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
