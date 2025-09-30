use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::event_stream::event_handler::EventHandler;
use crate::event_stream::event_stream_impl::EventStream;
use crate::event_stream::predicate::Predicate;
use nexus_message_derive_rs::Message;
use tokio::sync::Mutex;

#[derive(Debug, PartialEq, Eq, Message)]
pub struct TestString(pub String);

#[tokio::test]
async fn test_event_stream_subscribe() {
  let es = EventStream::new();
  let s = es.subscribe(|_| async move {}).await;
  assert!(s.is_active());
  assert_eq!(es.length(), 1);
}

#[tokio::test]
async fn test_event_stream_unsubscribe() {
  let es = EventStream::new();
  let c1 = Arc::new(AtomicI32::new(0));
  let c2 = Arc::new(AtomicI32::new(0));

  let s1 = es
    .subscribe({
      let c1 = Arc::clone(&c1);
      move |_| {
        let c1 = c1.clone();
        async move {
          c1.fetch_add(1, Ordering::SeqCst);
        }
      }
    })
    .await;
  let s2 = es
    .subscribe({
      let c2 = Arc::clone(&c2);
      move |_| {
        let c2 = c2.clone();
        async move {
          c2.fetch_add(1, Ordering::SeqCst);
        }
      }
    })
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
  es.subscribe(move |m| {
    let v_clone = v_clone.clone();
    let m_value = m.as_any().downcast_ref::<i32>().copied();
    async move {
      if let Some(val) = m_value {
        *v_clone.lock().await = val;
      }
    }
  })
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
    EventHandler::new(move |_| {
      let called_clone = called_clone.clone();
      async move {
        *called_clone.lock().await = true;
      }
    }),
    Predicate::new(|_| true),
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
    EventHandler::new(move |_| {
      let called_clone = called_clone.clone();
      async move {
        *called_clone.lock().await = true;
      }
    }),
    Predicate::new(|_: MessageHandle| false),
  )
  .await;
  es.publish(MessageHandle::new(TestString("".to_string()))).await;

  assert!(!*called.lock().await);
}

#[derive(Debug, PartialEq, Eq, Message)]
struct Event {
  i: i32,
}

#[tokio::test]
async fn test_event_stream_performance() {
  let es = EventStream::new();
  let mut subs = Vec::new();

  for i in 0..1000 {
    // Reduced iterations for faster test
    for _ in 0..10 {
      let sub = es
        .subscribe(move |evt| {
          let i = i; // Capture i by value
          let evt_data = evt.as_any().downcast_ref::<Event>().map(|e| e.i);
          async move {
            if let Some(evt_i) = evt_data {
              assert_eq!(evt_i, i, "expected i to be {} but its value is {}", i, evt_i);
            }
          }
        })
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
