#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod collections;
pub mod concurrent;
pub mod sync;

pub use collections::{
  Element, MpscBackend, MpscBuffer, MpscQueue, PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize,
  QueueStorage, QueueWriter, RingBuffer, RingBufferBackend, RingBufferStorage, RingQueue, SharedMpscHandle,
  SharedPriorityQueue, SharedQueue, SharedQueueHandle, SharedStack, SharedStackHandle, StackBase, StackBuffer,
  StackError, StackMut, StackStorage, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};
pub use concurrent::{
  AsyncBarrier, AsyncBarrierBackend, BoxFuture, CountDownLatch, CountDownLatchBackend, GuardHandle, Synchronized,
  SynchronizedMutexBackend, SynchronizedRw, SynchronizedRwBackend, WaitGroup, WaitGroupBackend,
};
pub use sync::{Shared, StateCell};
