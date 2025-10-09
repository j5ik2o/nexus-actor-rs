//! コアユーティリティ群。
//!
//! メールボックスや同期原語、期限付き処理など、ランタイム間で共有したい
//! 基本的なデータ構造を `no_std` 前提で提供する。`actor-core` とはこの crate
//! を介してやり取りすることで依存方向を一方向に保ち、各ランタイムはここで定義した
//! 抽象を自前の実装で満たすだけでよい。

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
