use core::future::Ready;
use core::time::Duration;

use nexus_actor_core_rs::Timer;

/// A timer that completes immediately.
///
/// A timer implementation for embedded environments that completes instantly without waiting.
pub struct ImmediateTimer;

impl Timer for ImmediateTimer {
  type SleepFuture<'a>
    = Ready<()>
  where
    Self: 'a;

  fn sleep(&self, _duration: Duration) -> Self::SleepFuture<'_> {
    core::future::ready(())
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use std::task::{Context, Poll};
  use std::{future::Future, pin::Pin, sync::Arc, task::Wake, task::Waker};

  fn noop_waker() -> Waker {
    struct NoopWake;
    impl Wake for NoopWake {
      fn wake(self: Arc<Self>) {}

      fn wake_by_ref(self: &Arc<Self>) {}
    }
    Waker::from(Arc::new(NoopWake))
  }

  #[test]
  fn immediate_timer_sleep_is_ready() {
    let timer = ImmediateTimer;
    let mut fut = timer.sleep(Duration::from_secs(1));
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(Pin::new(&mut fut).poll(&mut cx), Poll::Ready(()));
  }
}
