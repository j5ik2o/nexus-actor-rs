#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod collections;
pub mod sync;

pub use collections::{
  Element, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueStorage, QueueWriter, RingBuffer,
  SharedQueue, SharedQueueHandle, SharedRingQueue, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};
pub use sync::{Shared, StateCell};
