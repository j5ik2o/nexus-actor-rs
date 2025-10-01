#[cfg(test)]
mod test {
  use std::env;
  use std::time::{Duration, Instant};

  use rstest::*;
  use tracing_subscriber::EnvFilter;

  use crate::actor::core::RestartStatistics;
  use crate::actor::supervisor::exponential_backoff_strategy::ExponentialBackoffStrategy;

  #[tokio::test]
  async fn test_backoff_calculation() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let initial_backoff = Duration::from_millis(100);

    let mut rs = RestartStatistics::new();

    // 2回失敗を記録
    rs.fail().await;
    rs.fail().await;

    // バックオフ時間の計算をテスト
    let backoff_nanos = rs.failure_count().await as u64 * initial_backoff.as_nanos() as u64;
    assert!(backoff_nanos > 0);
    assert_eq!(backoff_nanos, 2 * initial_backoff.as_nanos() as u64);
  }

  #[rstest(ft, fc, expected)]
  #[case(11, 10, 1)]
  #[case(9, 10, 11)]
  #[tokio::test]
  async fn test_exponential_backoff_strategy_set_failure_count(ft: u64, fc: usize, expected: usize) {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let s = ExponentialBackoffStrategy::new(Duration::from_secs(10));
    let mut rs = RestartStatistics::new();
    for _ in 0..fc {
      rs.push(Instant::now() - Duration::from_secs(ft)).await;
    }

    s.set_failure_count(&mut rs).await;
    assert_eq!(expected, rs.failure_count().await);
  }

  #[tokio::test]
  async fn test_exponential_backoff_strategy_increments_failure_count() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let mut rs = RestartStatistics::new();
    let s = ExponentialBackoffStrategy::new(Duration::from_secs(10));

    s.set_failure_count(&mut rs).await;
    s.set_failure_count(&mut rs).await;
    s.set_failure_count(&mut rs).await;

    assert_eq!(3, rs.failure_count().await);
  }

  #[tokio::test]
  async fn test_exponential_backoff_strategy_resets_failure_count() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let mut rs = RestartStatistics::new();
    for _ in 0..10 {
      rs.push(Instant::now() - Duration::from_secs(11)).await;
    }
    let s = ExponentialBackoffStrategy::new(Duration::from_secs(10));

    s.set_failure_count(&mut rs).await;

    assert_eq!(1, rs.failure_count().await);
  }
}
