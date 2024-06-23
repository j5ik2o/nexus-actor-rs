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

pub struct ThrottleCallbackFunc(Arc<Mutex<dyn FnMut(usize) -> BoxFuture<'static, ()> + Send + Sync + 'static>>);

impl ThrottleCallbackFunc {
  pub fn new(f: impl FnMut(usize) -> BoxFuture<'static, ()> + Send + Sync + 'static) -> Self {
    ThrottleCallbackFunc(Arc::new(Mutex::new(f)))
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
    throttled_callback: ThrottleCallbackFunc,
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

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tokio::sync::Mutex;
  use tokio::time::Duration;

  use super::*;

  #[tokio::test]
  async fn test_throttler() {
    let callback_called = Arc::new(Mutex::new(false));
    let callback_called_clone = Arc::clone(&callback_called);

    let throttle = Throttle::new(
      10,
      Duration::from_millis(100),
      ThrottleCallbackFunc::new(move |_| {
        let callback_called = callback_called_clone.clone();
        Box::pin(async move {
          let mut called = callback_called.lock().await;
          *called = true;
        })
      }),
    )
    .await;

    assert_eq!(throttle.should_throttle(), Valve::Open);
    assert_eq!(throttle.should_throttle(), Valve::Open);

    for _ in 0..7 {
      throttle.should_throttle();
    }

    // 10回目の呼び出しでClosingになるはず
    assert_eq!(throttle.should_throttle(), Valve::Closing);

    // 11回目の呼び出しでClosedになるはず
    assert_eq!(throttle.should_throttle(), Valve::Closed);

    // コールバックが呼ばれるのを待つ
    tokio::time::sleep(Duration::from_millis(150)).await;

    // コールバックが呼ばれたことを確認
    assert!(*callback_called.lock().await);

    // 時間が経過した後、再びOpenになるはず
    assert_eq!(throttle.should_throttle(), Valve::Open);
  }
}
