use core::future::Ready;
use core::time::Duration;

use nexus_actor_core_rs::Timer;

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
