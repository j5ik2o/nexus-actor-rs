use core::future::Future;
use core::time::Duration;

/// Timer abstraction shared across runtimes.
pub trait Timer {
  type SleepFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  fn sleep(&self, duration: Duration) -> Self::SleepFuture<'_>;
}
