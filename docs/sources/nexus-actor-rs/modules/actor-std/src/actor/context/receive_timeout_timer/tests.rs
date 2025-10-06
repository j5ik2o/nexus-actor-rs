use crate::actor::context::receive_timeout_timer::ReceiveTimeoutTimer;
use std::time::Duration;
use tokio::time::{sleep, Instant};

#[tokio::test]
async fn test_timer_reset_before_expiration() {
  let mut timer = ReceiveTimeoutTimer::new(Duration::from_millis(100));
  timer.init(Instant::now() + Duration::from_millis(100)).await;
  sleep(Duration::from_millis(50)).await;
  timer.reset(Instant::now() + Duration::from_millis(100)).await;
  let start = Instant::now();
  timer.wait().await;
  assert!(start.elapsed() >= Duration::from_millis(100));
}

#[tokio::test]
async fn test_timer_stop() {
  let mut timer = ReceiveTimeoutTimer::new(Duration::from_millis(100));
  timer.init(Instant::now() + Duration::from_millis(100)).await;
  sleep(Duration::from_millis(50)).await;
  timer.stop().await;
  let start = Instant::now();
  timer.wait().await;
  assert!(start.elapsed() < Duration::from_millis(100));
}

#[tokio::test]
async fn test_timer_multiple_resets() {
  let mut timer = ReceiveTimeoutTimer::new(Duration::from_millis(50));
  timer.init(Instant::now() + Duration::from_millis(50)).await;

  for _ in 0..3 {
    sleep(Duration::from_millis(25)).await;
    timer.reset(Instant::now() + Duration::from_millis(50)).await;
  }

  let start = Instant::now();
  timer.wait().await;
  assert!(start.elapsed() >= Duration::from_millis(50));
}
