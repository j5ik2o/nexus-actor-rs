use core::future::Future;

use nexus_actor_core_rs::Spawn;

/// A spawner that immediately drops futures.
///
/// An implementation for embedded environments that simply drops tasks without actually executing them.
pub struct ImmediateSpawner;

impl Spawn for ImmediateSpawner {
  fn spawn(&self, _fut: impl Future<Output = ()> + 'static) {}
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::Arc;

  #[test]
  fn immediate_spawner_drops_future_without_polling() {
    let spawner = ImmediateSpawner;
    let polled = Arc::new(AtomicBool::new(false));
    let flag = polled.clone();

    spawner.spawn(async move {
      flag.store(true, Ordering::SeqCst);
    });

    assert!(
      !polled.load(Ordering::SeqCst),
      "future should not be polled by ImmediateSpawner"
    );
  }
}
