#![no_std]

extern crate alloc;

pub(crate) mod collections;
pub(crate) mod concurrent;
pub(crate) mod sync;

pub use nexus_utils_core_rs::{
  Element, MpscHandle, PriorityMessage, QueueBase, QueueError, QueueReader, QueueRw, QueueRwHandle, QueueSize,
  QueueStorage, QueueWriter, RingBackend, RingBuffer, RingQueue, RingStorageBackend, Shared, Stack, StackBackend,
  StackHandle, StackStorage, StackStorageBackend, StateCell, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};

pub use collections::*;
#[cfg(feature = "arc")]
pub use concurrent::{
  ArcAsyncBarrierBackend, ArcCountDownLatchBackend, ArcCsAsyncBarrier, ArcCsCountDownLatch, ArcCsSynchronized,
  ArcCsSynchronizedRw, ArcCsWaitGroup, ArcLocalAsyncBarrier, ArcLocalCountDownLatch, ArcLocalSynchronized,
  ArcLocalSynchronizedRw, ArcLocalWaitGroup, ArcMutexBackend, ArcRwLockBackend, ArcSynchronized, ArcSynchronizedRw,
  ArcWaitGroupBackend,
};
#[cfg(feature = "rc")]
pub use concurrent::{
  RcAsyncBarrier, RcAsyncBarrierBackend, RcCountDownLatch, RcCountDownLatchBackend, RcMutexBackend, RcRwLockBackend,
  RcSynchronized, RcSynchronizedRw, RcWaitGroup, RcWaitGroupBackend,
};
pub use sync::*;

pub mod prelude {
  #[cfg(feature = "arc")]
  pub use super::{
    ArcCsAsyncBarrier, ArcCsCountDownLatch, ArcCsSynchronized, ArcCsSynchronizedRw, ArcCsWaitGroup,
    ArcLocalAsyncBarrier, ArcLocalCountDownLatch, ArcLocalWaitGroup, ArcSynchronized, ArcSynchronizedRw,
  };
  #[cfg(feature = "arc")]
  pub use super::{
    ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcCsPriorityQueue, ArcCsStack, ArcLocalMpscBoundedQueue,
    ArcLocalMpscUnboundedQueue, ArcLocalPriorityQueue, ArcLocalRingQueue, ArcLocalStack, ArcMpscBoundedQueue,
    ArcMpscUnboundedQueue, ArcPriorityQueue, ArcRingQueue, ArcStack,
  };
  #[cfg(feature = "arc")]
  pub use super::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
  #[cfg(feature = "rc")]
  pub use super::{RcAsyncBarrier, RcCountDownLatch, RcSynchronized, RcSynchronizedRw, RcWaitGroup};
  #[cfg(feature = "rc")]
  pub use super::{RcMpscBoundedQueue, RcMpscUnboundedQueue, RcPriorityQueue, RcRingQueue, RcStack};
  #[cfg(feature = "rc")]
  pub use super::{RcShared, RcStateCell};
  pub use nexus_utils_core_rs::{
    Element, MpscHandle, PriorityMessage, QueueBase, QueueError, QueueReader, QueueRw, QueueRwHandle, QueueSize,
    QueueStorage, QueueWriter, RingBackend, RingBuffer, RingQueue, RingStorageBackend, Shared, Stack, StackBackend,
    StackBase, StackBuffer, StackError, StackHandle, StackMut, StackStorage, StackStorageBackend, StateCell,
    DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
  };
}

#[cfg(test)]
mod tests;
