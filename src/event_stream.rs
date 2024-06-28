use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::RwLock;

use crate::actor::message::{Message, MessageHandle};

// Handler defines a callback function that must be passed when subscribing.
#[derive(Clone)]
pub struct HandlerFunc(Arc<dyn Fn(MessageHandle) -> BoxFuture<'static, ()> + Send + Sync + 'static>);

impl Debug for HandlerFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Handler")
  }
}

impl PartialEq for HandlerFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for HandlerFunc {}

impl std::hash::Hash for HandlerFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(MessageHandle) -> BoxFuture<'static, ()>).hash(state);
  }
}

impl HandlerFunc {
  pub fn new(f: impl Fn(MessageHandle) -> BoxFuture<'static, ()> + Send + Sync + 'static) -> Self {
    HandlerFunc(Arc::new(f))
  }

  pub async fn run(&self, evt: MessageHandle) {
    (self.0)(evt).await
  }
}

// Predicate is a function used to filter messages before being forwarded to a subscriber
#[derive(Clone)]
pub struct PredicateFunc(Arc<dyn Fn(MessageHandle) -> bool + Send + Sync>);

impl Debug for PredicateFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Predicate")
  }
}

impl PartialEq for PredicateFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for PredicateFunc {}

impl std::hash::Hash for PredicateFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(MessageHandle) -> bool).hash(state);
  }
}

impl PredicateFunc {
  pub fn new(f: impl Fn(MessageHandle) -> bool + Send + Sync + 'static) -> Self {
    PredicateFunc(Arc::new(f))
  }

  pub fn run(&self, evt: MessageHandle) -> bool {
    (self.0)(evt)
  }
}

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

  pub async fn subscribe(&self, handler: HandlerFunc) -> Subscription {
    let sub = Subscription {
      id: self.counter.fetch_add(1, Ordering::SeqCst),
      handler: Arc::new(handler),
      predicate: None,
      active: Arc::new(AtomicU32::new(1)),
    };

    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.push(sub.clone());

    sub
  }

  pub async fn subscribe_with_predicate(&self, handler: HandlerFunc, predicate: PredicateFunc) -> Subscription {
    let sub = Subscription {
      id: self.counter.fetch_add(1, Ordering::SeqCst),
      handler: Arc::new(handler),
      predicate: Some(predicate),
      active: Arc::new(AtomicU32::new(1)),
    };

    let mut subscriptions = self.subscriptions.write().await;
    subscriptions.push(sub.clone());

    sub
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

#[derive(Debug, Clone)]
pub struct Subscription {
  id: i32,
  handler: Arc<HandlerFunc>,
  predicate: Option<PredicateFunc>,
  active: Arc<AtomicU32>,
}

unsafe impl Send for Subscription {}
unsafe impl Sync for Subscription {}

impl PartialEq for Subscription {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Eq for Subscription {}

impl Subscription {
  pub fn activate(&self) -> bool {
    self
      .active
      .compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed)
      .is_ok()
  }

  pub fn deactivate(&self) -> bool {
    self
      .active
      .compare_exchange(1, 0, Ordering::SeqCst, Ordering::Relaxed)
      .is_ok()
  }

  pub fn is_active(&self) -> bool {
    self.active.load(Ordering::SeqCst) == 1
  }
}
