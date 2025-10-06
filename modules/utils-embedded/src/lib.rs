#![no_std]

extern crate alloc;

pub mod collections;
pub mod concurrent;
pub mod sync;

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
  pub use crate::collections::{
    ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcCsPriorityQueue, ArcCsStack, ArcLocalMpscBoundedQueue,
    ArcLocalMpscUnboundedQueue, ArcLocalPriorityQueue, ArcLocalRingQueue, ArcLocalStack, ArcMpscBoundedQueue,
    ArcMpscUnboundedQueue, ArcPriorityQueue, ArcRingQueue, ArcStack,
  };
  #[cfg(feature = "rc")]
  pub use crate::collections::{RcMpscBoundedQueue, RcMpscUnboundedQueue, RcPriorityQueue, RcRingQueue, RcStack};
  #[cfg(feature = "arc")]
  pub use crate::concurrent::{
    ArcCsAsyncBarrier, ArcCsCountDownLatch, ArcCsSynchronized, ArcCsSynchronizedRw, ArcCsWaitGroup,
    ArcLocalAsyncBarrier, ArcLocalCountDownLatch, ArcLocalWaitGroup, ArcSynchronized, ArcSynchronizedRw,
  };
  #[cfg(feature = "rc")]
  pub use crate::concurrent::{RcAsyncBarrier, RcCountDownLatch, RcSynchronized, RcSynchronizedRw, RcWaitGroup};
  #[cfg(feature = "arc")]
  pub use crate::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
  #[cfg(feature = "rc")]
  pub use crate::sync::{RcShared, RcStateCell};
  pub use nexus_utils_core_rs::{
    Element, MpscHandle, PriorityMessage, QueueBase, QueueError, QueueReader, QueueRw, QueueRwHandle, QueueSize,
    QueueStorage, QueueWriter, RingBackend, RingBuffer, RingQueue, RingStorageBackend, Shared, Stack, StackBackend,
    StackBase, StackBuffer, StackError, StackHandle, StackMut, StackStorage, StackStorageBackend, StateCell,
    DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
  };
}

#[cfg(test)]
mod tests;
