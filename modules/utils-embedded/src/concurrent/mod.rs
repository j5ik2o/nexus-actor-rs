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
#[cfg(feature = "rc")]
mod rc_wait_group;
#[cfg(feature = "rc")]
pub use rc_wait_group::{RcWaitGroupBackend, WaitGroup as RcWaitGroup};
#[cfg(feature = "rc")]
mod rc_async_barrier;
#[cfg(feature = "rc")]
pub use rc_async_barrier::{AsyncBarrier as RcAsyncBarrier, RcAsyncBarrierBackend};

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
#[cfg(feature = "arc")]
mod arc_wait_group;
#[cfg(feature = "arc")]
pub use arc_wait_group::{ArcCsWaitGroup, ArcLocalWaitGroup, ArcWaitGroupBackend};
#[cfg(feature = "arc")]
mod arc_async_barrier;
#[cfg(feature = "arc")]
pub use arc_async_barrier::{ArcAsyncBarrierBackend, ArcCsAsyncBarrier, ArcLocalAsyncBarrier};
