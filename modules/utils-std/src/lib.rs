//! std ランタイム向けユーティリティ。
//!
//! `nexus_utils_core_rs` で定義された抽象を tokio ベースの実装に結び付け、
//! `Arc` バックエンドや同期原語、期限付きタイマーをまとめて公開する。
//! core 側との循環を避けるため re-export を中心とした構成にしており、
//! `TokioDeadlineTimer` もここから提供される。

pub(crate) mod collections;
pub(crate) mod concurrent;
pub(crate) mod sync;
pub(crate) mod timing;

pub use nexus_utils_core_rs::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, Element,
  MpscHandle, PriorityMessage, QueueBase, QueueError, QueueHandle, QueueReader, QueueRw, QueueRwHandle, QueueSize,
  QueueStorage, QueueWriter, RingBackend, RingBuffer, RingQueue, RingStorageBackend, Shared, Stack, StackBackend,
  StackHandle, StackStorage, StackStorageBackend, StateCell, TimerDeadline, DEFAULT_CAPACITY, DEFAULT_PRIORITY,
  PRIORITY_LEVELS,
};

pub use collections::{ArcMpscBoundedQueue, ArcMpscUnboundedQueue, ArcPriorityQueue, ArcRingQueue, ArcStack};
pub use concurrent::{
  AsyncBarrier, CountDownLatch, Synchronized, SynchronizedRw, TokioAsyncBarrierBackend, TokioCountDownLatchBackend,
  TokioMutexBackend, TokioRwLockBackend, TokioWaitGroupBackend, WaitGroup,
};
pub use sync::{ArcShared, ArcStateCell};
pub use timing::TokioDeadlineTimer;

pub mod prelude {
  pub use super::{
    ArcMpscBoundedQueue, ArcMpscUnboundedQueue, ArcPriorityQueue, ArcRingQueue, ArcShared, ArcStack, ArcStateCell,
    AsyncBarrier, CountDownLatch, Synchronized, SynchronizedRw, TokioDeadlineTimer, WaitGroup,
  };
  pub use nexus_utils_core_rs::{
    DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, Element,
    MpscHandle, PriorityMessage, QueueBase, QueueError, QueueReader, QueueRw, QueueRwHandle, QueueSize, QueueStorage,
    QueueWriter, RingBackend, RingBuffer, RingStorageBackend, Shared, Stack, StackBackend, StackHandle, StackStorage,
    StackStorageBackend, StateCell, TimerDeadline, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
  };
}
