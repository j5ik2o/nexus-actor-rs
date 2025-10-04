use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::runtime::StdAsyncMutex;
use nexus_actor_core_rs::runtime::{AsyncMutex, CoreScheduledHandleRef, CoreScheduledTask, CoreScheduler};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Valve {
  Open,
  Closing,
  Closed,
}

#[derive(Clone)]
pub struct Throttle {
  current_events: Arc<AtomicUsize>,
  max_events_in_period: usize,
  _worker_handle: CoreScheduledHandleRef,
}

impl std::fmt::Debug for Throttle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Throttle")
      .field("max_events_in_period", &self.max_events_in_period)
      .finish()
  }
}

impl Throttle {
  pub async fn new<F, Fut>(
    scheduler: Arc<dyn CoreScheduler>,
    max_events_in_period: usize,
    period: Duration,
    throttled_callback: F,
  ) -> Arc<Self>
  where
    F: Fn(usize) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    let events = Arc::new(AtomicUsize::new(0));
    let callback = make_task(events.clone(), max_events_in_period, throttled_callback);
    let handle = scheduler.schedule_repeated(Duration::ZERO, period, callback);

    Arc::new(Self {
      current_events: events,
      max_events_in_period,
      _worker_handle: handle,
    })
  }

  pub fn should_throttle(&self) -> Valve {
    let tries = self.current_events.fetch_add(1, Ordering::SeqCst) + 1;
    match tries.cmp(&self.max_events_in_period) {
      std::cmp::Ordering::Less => Valve::Open,
      std::cmp::Ordering::Equal => Valve::Closing,
      std::cmp::Ordering::Greater => Valve::Closed,
    }
  }
}

fn make_task<F, Fut>(
  events: Arc<AtomicUsize>,
  max_events_in_period: usize,
  throttled_callback: F,
) -> CoreScheduledTask
where
  F: Fn(usize) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = ()> + Send + 'static, {
  let callback = Arc::new(StdAsyncMutex::new(throttled_callback));

  Arc::new(move || {
    let events = events.clone();
    let callback = callback.clone();
    Box::pin(async move {
      let times_called = events.swap(0, Ordering::SeqCst);
      if times_called > max_events_in_period {
        let excess = times_called - max_events_in_period;
        let cb = &mut *callback.lock().await;
        cb(excess).await;
      }
    })
  })
}
