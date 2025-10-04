use core::time::Duration;

use nexus_actor_core_rs::Timer;
use tokio::time::Sleep;

/// Tokio-backed timer implementation.
pub struct TokioTimer;

impl Timer for TokioTimer {
  type SleepFuture<'a>
    = Sleep
  where
    Self: 'a;

  fn sleep(&self, duration: Duration) -> Self::SleepFuture<'_> {
    tokio::time::sleep(duration)
  }
}
