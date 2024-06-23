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

#[cfg(test)]
mod tests {
  use std::any::Any;

  use tokio::sync::Mutex;

  use super::*;

  #[derive(Debug)]
  pub struct TestString(pub String);
  impl Message for TestString {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[tokio::test]
  async fn test_event_stream_subscribe() {
    let es = EventStream::new();
    let s = es.subscribe(HandlerFunc::new(|_| Box::pin(async move {}))).await;
    assert!(s.is_active());
    assert_eq!(es.length(), 1);
  }

  #[tokio::test]
  async fn test_event_stream_unsubscribe() {
    let es = EventStream::new();
    let c1 = Arc::new(AtomicI32::new(0));
    let c2 = Arc::new(AtomicI32::new(0));

    let s1 = es
      .subscribe(HandlerFunc::new({
        let c1 = Arc::clone(&c1);
        move |_| {
          let c1 = c1.clone();
          Box::pin(async move {
            c1.fetch_add(1, Ordering::SeqCst);
          })
        }
      }))
      .await;
    let s2 = es
      .subscribe(HandlerFunc::new({
        let c2 = Arc::clone(&c2);
        move |_| {
          let c2 = c2.clone();
          Box::pin(async move {
            c2.fetch_add(1, Ordering::SeqCst);
          })
        }
      }))
      .await;
    assert_eq!(es.length(), 2);

    es.unsubscribe(s2).await;
    assert_eq!(es.length(), 1);

    es.publish(MessageHandle::new(1)).await;
    assert_eq!(c1.load(Ordering::SeqCst), 1);

    es.unsubscribe(s1).await;
    assert_eq!(es.length(), 0);

    es.publish(MessageHandle::new(1)).await;
    assert_eq!(c1.load(Ordering::SeqCst), 1);
    assert_eq!(c2.load(Ordering::SeqCst), 0);
  }

  #[tokio::test]
  async fn test_event_stream_publish() {
    let es = EventStream::new();
    let v = Arc::new(Mutex::new(0));

    let v_clone = Arc::clone(&v);
    es.subscribe(HandlerFunc::new(move |m| {
      let v_clone = v_clone.clone();
      let m_value = if let Some(val) = m.as_any().downcast_ref::<i32>() {
        Some(*val)
      } else {
        None
      };
      Box::pin(async move {
        if let Some(val) = m_value {
          *v_clone.lock().await = val;
        }
      })
    }))
    .await;

    es.publish(MessageHandle::new(1)).await;
    assert_eq!(*v.lock().await, 1);

    es.publish(MessageHandle::new(100)).await;
    assert_eq!(*v.lock().await, 100);
  }

  #[tokio::test]
  async fn test_event_stream_subscribe_with_predicate_is_called() {
    let es = EventStream::new();
    let called = Arc::new(Mutex::new(false));

    let called_clone = Arc::clone(&called);
    es.subscribe_with_predicate(
      HandlerFunc::new(move |_| {
        let called_clone = called_clone.clone();
        Box::pin(async move {
          *called_clone.lock().await = true;
        })
      }),
      PredicateFunc::new(|_| true),
    )
    .await;
    es.publish(MessageHandle::new(TestString("".to_string()))).await;

    assert!(*called.lock().await);
  }

  #[tokio::test]
  async fn test_event_stream_subscribe_with_predicate_is_not_called() {
    let es = EventStream::new();
    let called = Arc::new(Mutex::new(false));

    let called_clone = Arc::clone(&called);
    es.subscribe_with_predicate(
      HandlerFunc::new(move |_| {
        let called_clone = called_clone.clone();
        Box::pin(async move {
          *called_clone.lock().await = true;
        })
      }),
      PredicateFunc::new(|_: MessageHandle| false),
    )
    .await;
    es.publish(MessageHandle::new(TestString("".to_string()))).await;

    assert!(!*called.lock().await);
  }

  #[derive(Debug)]
  struct Event {
    i: i32,
  }

  impl Message for Event {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[tokio::test]
  async fn test_event_stream_performance() {
    let es = EventStream::new();
    let mut subs = Vec::new();

    for i in 0..1000 {
      // Reduced iterations for faster test
      for _ in 0..10 {
        let sub = es
          .subscribe(HandlerFunc::new(move |evt| {
            let i = i; // Capture i by value
            let evt_data = if let Some(e) = evt.as_any().downcast_ref::<Event>() {
              Some(e.i)
            } else {
              None
            };
            Box::pin(async move {
              if let Some(evt_i) = evt_data {
                assert_eq!(evt_i, i, "expected i to be {} but its value is {}", i, evt_i);
              }
            })
          }))
          .await;
        subs.push(sub);
      }

      es.publish(MessageHandle::new(Event { i })).await;
      for sub in subs.drain(..) {
        es.unsubscribe(sub.clone()).await;
        assert!(!sub.is_active(), "subscription should not be active");
      }
    }
  }
}
