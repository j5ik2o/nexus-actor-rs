#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub(crate) mod collections;
pub(crate) mod concurrent;
pub(crate) mod sync;
pub(crate) mod timing;

pub use async_trait::async_trait;

pub use collections::{
  Element, MpscBackend, MpscBuffer, MpscHandle, MpscQueue, PriorityMessage, PriorityQueue, QueueBase, QueueError,
  QueueHandle, QueueReader, QueueRw, QueueRwHandle, QueueSize, QueueStorage, QueueWriter, RingBackend, RingBuffer,
  RingBufferBackend, RingBufferStorage, RingHandle, RingQueue, RingStorageBackend, Stack, StackBackend, StackBase,
  StackBuffer, StackError, StackHandle, StackMut, StackStorage, StackStorageBackend, DEFAULT_CAPACITY,
  DEFAULT_PRIORITY, PRIORITY_LEVELS,
};
pub use concurrent::{
  AsyncBarrier, AsyncBarrierBackend, CountDownLatch, CountDownLatchBackend, GuardHandle, Synchronized,
  SynchronizedMutexBackend, SynchronizedRw, SynchronizedRwBackend, WaitGroup, WaitGroupBackend,
};
pub use sync::{Flag, Shared, StateCell};
pub use timing::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, TimerDeadline,
};
