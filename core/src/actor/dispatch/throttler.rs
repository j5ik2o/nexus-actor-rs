use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::actor::dispatch::{Dispatcher, Runnable};
use tokio::time::{interval, Duration};

#[cfg(test)]
mod tests;

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

impl Throttle {
  pub async fn new<F, Fut>(
    dispatcher: Arc<dyn Dispatcher>,
    max_events_in_period: usize,
    period: Duration,
    mut throttled_callback: F,
  ) -> Arc<Self>
  where
    F: FnMut(usize) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    let throttle = Arc::new(Self {
      current_events: Arc::new(AtomicUsize::new(0)),
      max_events_in_period,
    });

    let throttle_clone = Arc::clone(&throttle);

    dispatcher
      .schedule(Runnable::new(move || async move {
        let mut interval = interval(period);
        loop {
          interval.tick().await;
          let times_called = throttle_clone.current_events.swap(0, Ordering::SeqCst);
          if times_called > max_events_in_period {
            throttled_callback(times_called - max_events_in_period).await;
          }
        }
      }))
      .await;

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
