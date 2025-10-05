#![no_std]

extern crate alloc;

pub mod collections;
pub mod sync;

pub use nexus_utils_core_rs::{
  Element, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueStorage, QueueWriter, RingBuffer,
  Shared, SharedQueue, SharedQueueHandle, SharedRingQueue, StateCell, DEFAULT_CAPACITY, DEFAULT_PRIORITY,
  PRIORITY_LEVELS,
};

pub use collections::*;
pub use sync::*;

pub mod prelude {
  #[cfg(feature = "arc")]
  pub use crate::collections::{
    ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcCsPriorityQueue, ArcLocalMpscBoundedQueue,
    ArcLocalMpscUnboundedQueue, ArcLocalPriorityQueue, ArcLocalRingQueue, ArcMpscBoundedQueue, ArcMpscUnboundedQueue,
    ArcPriorityQueue, ArcRingQueue,
  };
  #[cfg(feature = "rc")]
  pub use crate::collections::{RcMpscBoundedQueue, RcMpscUnboundedQueue, RcPriorityQueue, RcRingQueue};
  #[cfg(feature = "arc")]
  pub use crate::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
  #[cfg(feature = "rc")]
  pub use crate::sync::{RcShared, RcStateCell};
  pub use nexus_utils_core_rs::{
    Element, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueStorage, QueueWriter, RingBuffer,
    Shared, SharedQueue, SharedQueueHandle, SharedRingQueue, StateCell, DEFAULT_CAPACITY, DEFAULT_PRIORITY,
    PRIORITY_LEVELS,
  };
}

#[cfg(test)]
mod tests;
