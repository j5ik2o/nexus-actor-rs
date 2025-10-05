#![no_std]

extern crate alloc;

pub mod collections;
pub mod concurrent;
pub mod sync;

pub use nexus_utils_core_rs::{
  Element, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueStorage, QueueWriter, RingBuffer,
  Shared, SharedQueue, SharedQueueHandle, SharedRingQueue, StateCell, DEFAULT_CAPACITY, DEFAULT_PRIORITY,
  PRIORITY_LEVELS,
};

pub use collections::*;
#[cfg(feature = "arc")]
pub use concurrent::{
  ArcCsSynchronized, ArcCsSynchronizedRw, ArcLocalSynchronized, ArcLocalSynchronizedRw, ArcMutexBackend,
  ArcRwLockBackend, ArcSynchronized, ArcSynchronizedRw,
};
#[cfg(feature = "rc")]
pub use concurrent::{RcMutexBackend, RcRwLockBackend, RcSynchronized, RcSynchronizedRw};
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
    ArcCsSynchronized, ArcCsSynchronizedRw, ArcLocalSynchronized, ArcLocalSynchronizedRw, ArcSynchronized,
    ArcSynchronizedRw,
  };
  #[cfg(feature = "rc")]
  pub use crate::concurrent::{RcSynchronized, RcSynchronizedRw};
  #[cfg(feature = "arc")]
  pub use crate::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
  #[cfg(feature = "rc")]
  pub use crate::sync::{RcShared, RcStateCell};
  pub use nexus_utils_core_rs::{
    Element, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueStorage, QueueWriter, RingBuffer,
    Shared, SharedQueue, SharedQueueHandle, SharedRingQueue, SharedStack, SharedStackHandle, StackBase, StackBuffer,
    StackError, StackMut, StackStorage, StateCell, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
  };
}

#[cfg(test)]
mod tests;
