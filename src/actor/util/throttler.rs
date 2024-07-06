use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Valve {
  Open,
  Closing,
  Closed,
}

#[derive(Debug, Clone)]
pub struct Throttle {
  current_events: Arc<AtomicUsize>,
  max_events_in_period: usize,
}

pub struct ThrottleCallback(Arc<Mutex<dyn FnMut(usize) -> BoxFuture<'static, ()> + Send + 'static>>);

impl ThrottleCallback {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(usize) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Arc::new(Mutex::new(move |size: usize| {
      Box::pin(f(size)) as BoxFuture<'static, ()>
    })))
  }

  pub async fn run(&self, times_called: usize) {
    let mut f = self.0.lock().await;
    f(times_called).await;
  }
}

impl Throttle {
  pub async fn new(
    max_events_in_period: usize,
    period: Duration,
    throttled_callback: ThrottleCallback,
  ) -> Arc<Self> {
    let throttle = Arc::new(Self {
      current_events: Arc::new(AtomicUsize::new(0)),
      max_events_in_period,
    });

    let throttle_clone = Arc::clone(&throttle);
    tokio::spawn(async move {
      let mut interval = interval(period);
      loop {
        interval.tick().await;
        let times_called = throttle_clone.current_events.swap(0, Ordering::SeqCst);
        if times_called > max_events_in_period {
          throttled_callback.run(times_called - max_events_in_period).await;
        }
      }
    });

    throttle
  }

  pub fn should_throttle(&self) -> Valve {
    let tries = self.current_events.fetch_add(1, Ordering::SeqCst) + 1;
    if tries == self.max_events_in_period {
      Valve::Closing
    } else if tries > self.max_events_in_period {
      Valve::Closed
    } else {
      Valve::Open
    }
  }
}
