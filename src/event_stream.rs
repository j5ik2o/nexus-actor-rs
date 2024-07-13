pub mod handler;
pub mod predicate;
pub mod subscription;

use std::future::Future;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::actor::message::message_handle::MessageHandle;
use crate::event_stream::handler::Handler;
use crate::event_stream::predicate::Predicate;
use crate::event_stream::subscription::Subscription;

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

  pub async fn subscribe(&self, handler: Handler) -> Subscription {
    let subscription = Subscription::new(self.counter.fetch_add(1, Ordering::SeqCst), Arc::new(handler), None);
    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.push(subscription.clone());
    subscription
  }

  pub async fn subscribe_f<F, Fut>(&self, f: F) -> Subscription
  where
    F: Fn(MessageHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    self.subscribe(Handler::new(f)).await
  }

  pub async fn subscribe_with_predicate(&self, handler: Handler, predicate: Predicate) -> Subscription {
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
