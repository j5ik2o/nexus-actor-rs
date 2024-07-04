use std::time::{Duration, Instant};

use rstest::*;

use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::supervisor::exponential_backoff_strategy::ExponentialBackoffStrategy;

#[rstest(ft, fc, expected)]
#[case(11, 10, 1)]
#[case(9, 10, 11)]
#[tokio::test]
async fn test_exponential_backoff_strategy_set_failure_count(ft: u64, fc: usize, expected: usize) {
  let s = ExponentialBackoffStrategy::new(Duration::from_secs(10), None);
  let mut rs = RestartStatistics::new();
  for _ in 0..fc {
    rs.push(Instant::now() - Duration::from_secs(ft)).await;
  }

  s.set_failure_count(&mut rs).await;
  assert_eq!(expected, rs.failure_count().await);
}

#[tokio::test]
async fn test_exponential_backoff_strategy_increments_failure_count() {
  let mut rs = RestartStatistics::new();
  let s = ExponentialBackoffStrategy::new(Duration::from_secs(10), None);

  s.set_failure_count(&mut rs).await;
  s.set_failure_count(&mut rs).await;
  s.set_failure_count(&mut rs).await;

  assert_eq!(3, rs.failure_count().await);
}

#[tokio::test]
async fn test_exponential_backoff_strategy_resets_failure_count() {
  let mut rs = RestartStatistics::new();
  for _ in 0..10 {
    rs.push(Instant::now() - Duration::from_secs(11)).await;
  }
  let s = ExponentialBackoffStrategy::new(Duration::from_secs(10), Some(Duration::from_secs(1)));

  s.set_failure_count(&mut rs).await;

  assert_eq!(1, rs.failure_count().await);
}
