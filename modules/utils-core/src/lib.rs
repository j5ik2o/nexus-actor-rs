#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod collections;
pub mod concurrent;
pub mod sync;

pub use collections::{
  Element, MpscBackend, MpscBuffer, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueStorage,
  QueueWriter, RingBuffer, RingBufferBackend, RingBufferStorage, SharedMpscHandle, SharedMpscQueue,
  SharedPriorityQueue, SharedQueue, SharedQueueHandle, SharedRingQueue, SharedStack, SharedStackHandle, StackBase,
  StackBuffer, StackError, StackMut, StackStorage, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};
pub use concurrent::{BoxFuture, Synchronized, SynchronizedMutexBackend, SynchronizedRw, SynchronizedRwBackend};
pub use sync::{Shared, StateCell};
