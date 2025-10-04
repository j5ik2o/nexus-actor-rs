use core::future::Future;

/// Abstraction for scheduling asynchronous tasks.
pub trait Spawn {
  fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static);
}
