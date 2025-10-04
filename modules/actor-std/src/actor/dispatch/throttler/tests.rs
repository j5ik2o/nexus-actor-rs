use crate::actor::dispatch::throttler::{Throttle, Valve};
use crate::runtime::tokio_core_runtime;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_throttler() {
  let callback_called = Arc::new(Mutex::new(false));
  let callback_called_clone = Arc::clone(&callback_called);
  let scheduler = tokio_core_runtime().scheduler();

  let throttle = Throttle::new(scheduler, 10, Duration::from_millis(100), move |_| {
    let callback_called = callback_called_clone.clone();
    async move {
      let mut called = callback_called.lock().await;
      *called = true;
    }
  })
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
