#[cfg(feature = "rc")]
mod rc_synchronized;
#[cfg(feature = "rc")]
pub use rc_synchronized::{
  RcMutexBackend, RcRwLockBackend, Synchronized as RcSynchronized, SynchronizedRw as RcSynchronizedRw,
};
#[cfg(feature = "rc")]
mod rc_count_down_latch;
#[cfg(feature = "rc")]
pub use rc_count_down_latch::{CountDownLatch as RcCountDownLatch, RcCountDownLatchBackend};

#[cfg(feature = "arc")]
mod arc_synchronized;
#[cfg(feature = "arc")]
pub use arc_synchronized::{
  ArcCsSynchronized, ArcCsSynchronizedRw, ArcLocalSynchronized, ArcLocalSynchronizedRw, ArcMutexBackend,
  ArcRwLockBackend, ArcSynchronized, ArcSynchronizedRw,
};
#[cfg(feature = "arc")]
mod arc_count_down_latch;
#[cfg(feature = "arc")]
pub use arc_count_down_latch::{ArcCountDownLatchBackend, ArcCsCountDownLatch, ArcLocalCountDownLatch};
