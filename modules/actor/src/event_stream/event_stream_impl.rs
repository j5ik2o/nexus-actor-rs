use crate::actor::message::MessageHandle;
use crate::event_stream::event_handler::EventHandler;
use crate::event_stream::predicate::Predicate;
use crate::event_stream::subscription::Subscription;
use std::future::Future;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct EventStream {
  subscriptions: Arc<RwLock<Vec<Subscription>>>,
  counter: Arc<AtomicI32>,
}

impl EventStream {
  pub fn new() -> Self {
    EventStream {
      subscriptions: Arc::new(RwLock::new(Vec::new())),
      counter: Arc::new(AtomicI32::new(0)),
    }
  }

  pub async fn subscribe_handler(&self, handler: EventHandler) -> Subscription {
    let subscription = Subscription::new(self.counter.fetch_add(1, Ordering::SeqCst), Arc::new(handler), None);
    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.push(subscription.clone());
    subscription
  }

  pub async fn subscribe<F, Fut>(&self, f: F) -> Subscription
  where
    F: Fn(MessageHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    self.subscribe_handler(EventHandler::new(f)).await
  }

  pub async fn subscribe_with_predicate(&self, handler: EventHandler, predicate: Predicate) -> Subscription {
    let subscription = Subscription::new(
      self.counter.fetch_add(1, Ordering::SeqCst),
      Arc::new(handler),
      Some(predicate),
    );
    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.push(subscription.clone());
    subscription
  }

  pub async fn unsubscribe(&self, sub: Subscription) {
    if sub.is_active() {
      let mut subscriptions = self.subscriptions.write().await;
      if sub.deactivate() {
        if let Some(index) = subscriptions.iter().position(|s| *s == sub) {
          subscriptions.swap_remove(index);
          self.counter.fetch_sub(1, Ordering::SeqCst);
        }
      }
    }
  }

  pub async fn publish(&self, evt: MessageHandle) {
    let subscriptions = self.subscriptions.read().await;
    for sub in &*subscriptions {
      if let Some(predicate) = &sub.predicate {
        if !predicate.run(evt.clone()) {
          continue;
        }
      }
      sub.handler.run(evt.clone()).await;
    }
  }

  pub fn length(&self) -> i32 {
    self.counter.load(Ordering::SeqCst)
  }
}

impl Default for EventStream {
  fn default() -> Self {
    Self::new()
  }
}
