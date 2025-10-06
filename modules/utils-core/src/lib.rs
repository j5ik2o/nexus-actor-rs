#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod collections;
pub mod concurrent;
pub mod sync;

pub use collections::{
  Element, MpscBackend, MpscBuffer, MpscHandle, MpscQueue, PriorityMessage, QueueBase, QueueError, QueueHandle,
  QueueReader, QueueRw, QueueRwHandle, QueueSize, QueueStorage, QueueWriter, RingBackend, RingBuffer,
  RingBufferBackend, RingBufferStorage, RingHandle, RingQueue, RingStorageBackend, SharedPriorityQueue, Stack,
  StackBackend, StackBase, StackBuffer, StackError, StackHandle, StackMut, StackStorage, StackStorageBackend,
  DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};
pub use concurrent::{
  AsyncBarrier, AsyncBarrierBackend, BoxFuture, CountDownLatch, CountDownLatchBackend, GuardHandle, Synchronized,
  SynchronizedMutexBackend, SynchronizedRw, SynchronizedRwBackend, WaitGroup, WaitGroupBackend,
};
pub use sync::{Flag, Shared, StateCell};
