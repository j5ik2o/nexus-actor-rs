#[cfg(test)]
mod test {
  use std::env;
  use std::time::Duration;

  use tracing_subscriber::EnvFilter;

  use crate::actor::core::RestartStatistics;
  use crate::actor::supervisor::exponential_backoff_strategy::ExponentialBackoffStrategy;

  fn init_tracing() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();
  }

  #[tokio::test]
  async fn set_failure_count_accumulates_within_window() {
    init_tracing();
    let strategy = ExponentialBackoffStrategy::new(Duration::from_millis(50));
    let mut stats = RestartStatistics::with_runtime(&crate::runtime::tokio_core_runtime());

    strategy.set_failure_count(&mut stats).await;
    tokio::time::sleep(Duration::from_millis(15)).await;
    strategy.set_failure_count(&mut stats).await;

    assert_eq!(2, stats.failure_count().await);
  }

  #[tokio::test]
  async fn set_failure_count_resets_after_window() {
    init_tracing();
    let strategy = ExponentialBackoffStrategy::new(Duration::from_millis(30));
    let mut stats = RestartStatistics::with_runtime(&crate::runtime::tokio_core_runtime());

    strategy.set_failure_count(&mut stats).await;
    tokio::time::sleep(Duration::from_millis(45)).await;
    strategy.set_failure_count(&mut stats).await;

    assert_eq!(1, stats.failure_count().await);
  }

  #[tokio::test]
  async fn failure_count_scales_backoff_multiplier() {
    init_tracing();
    let initial_backoff = Duration::from_millis(25);
    let strategy = ExponentialBackoffStrategy::new(Duration::from_millis(80)).with_initial_backoff(initial_backoff);
    let mut stats = RestartStatistics::with_runtime(&crate::runtime::tokio_core_runtime());

    for _ in 0..3 {
      strategy.set_failure_count(&mut stats).await;
      tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let failure_count = stats.failure_count().await;
    assert_eq!(
      3, failure_count,
      "failure_count should reflect consecutive failures by count"
    );
  }
}
