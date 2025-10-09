#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_clone)]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enum)]
#![deny(clippy::no_effect)]
#![deny(clippy::drop_copy)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::wrong_pub_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]

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
