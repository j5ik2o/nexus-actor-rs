use nexus_actor_core_rs::runtime::CoreScheduledHandleRef;
use std::sync::Arc;

#[derive(Clone)]
pub struct ReceiveTimeoutTimer {
  handle: CoreScheduledHandleRef,
}

impl core::fmt::Debug for ReceiveTimeoutTimer {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ReceiveTimeoutTimer")
      .field("handle", &format_args!("{:p}", Arc::as_ptr(&self.handle)))
      .finish()
  }
}

impl ReceiveTimeoutTimer {
  pub fn new(handle: CoreScheduledHandleRef) -> Self {
    Self { handle }
  }

  pub fn cancel(&self) {
    self.handle.cancel();
  }

  pub fn is_cancelled(&self) -> bool {
    self.handle.is_cancelled()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_actor_core_rs::runtime::CoreScheduledTask;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::Arc;
  use std::time::Duration;

  #[tokio::test]
  async fn cancel_prevents_execution() {
    let factory = crate::runtime::tokio_core_runtime();
    let scheduler = runtime.scheduler();
    let fired = Arc::new(AtomicBool::new(false));
    let fired_clone = fired.clone();

    let task: CoreScheduledTask = Arc::new(move || {
      let fired = fired_clone.clone();
      Box::pin(async move {
        fired.store(true, Ordering::SeqCst);
      })
    });

    let handle = scheduler.schedule_once(Duration::from_millis(50), task);
    let timer = ReceiveTimeoutTimer::new(handle);
    timer.cancel();

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!fired.load(Ordering::SeqCst));
  }

  #[tokio::test]
  async fn handle_fires_when_not_cancelled() {
    let factory = crate::runtime::tokio_core_runtime();
    let scheduler = runtime.scheduler();
    let fired = Arc::new(AtomicBool::new(false));
    let fired_clone = fired.clone();

    let task: CoreScheduledTask = Arc::new(move || {
      let fired = fired_clone.clone();
      Box::pin(async move {
        fired.store(true, Ordering::SeqCst);
      })
    });

    let handle = scheduler.schedule_once(Duration::from_millis(50), task);
    let _timer = ReceiveTimeoutTimer::new(handle);

    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(fired.load(Ordering::SeqCst));
  }
}
