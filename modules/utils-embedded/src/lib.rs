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

//! 組み込み環境向けユーティリティライブラリ。
//!
//! このクレートは、`no_std` 環境で動作する並行処理および同期プリミティブを提供します。
//! `actor-embedded` などのランタイムが、標準ライブラリなしで動作できるよう設計されています。
//!
//! # 主な機能
//!
//! ## 同期プリミティブ
//!
//! - **AsyncBarrier**: 複数のタスクが同期ポイントで待機するバリア機構
//! - **CountDownLatch**: カウントダウンベースのラッチ（カウントが 0 になるまで待機）
//! - **WaitGroup**: 複数のタスクの完了を追跡する待機グループ
//! - **Synchronized**: 排他的アクセス制御を提供する同期型（Mutex ベース）
//! - **SynchronizedRw**: 読み取り/書き込みアクセス制御を提供する同期型（RwLock ベース）
//!
//! ## コレクション
//!
//! - **Queue**: 有界/無界キュー、優先度付きキュー、リングバッファ
//! - **Stack**: スタック構造
//! - **MPSC**: マルチプロデューサー/シングルコンシューマーキュー
//!
//! ## タイマー
//!
//! - **ManualDeadlineTimer**: ソフトウェア歩進によるデッドラインタイマー
//!
//! # 所有権モデル
//!
//! 所有権モデルは機能フラグで切り替え可能です：
//!
//! - **`rc` フィーチャ**: `Rc` ベースの実装（シングルスレッド、デフォルト）
//! - **`arc` フィーチャ**: `Arc` ベースの実装（マルチスレッド対応）
//!
//! # 使用例
//!
//! ```ignore
//! use nexus_utils_embedded_rs::prelude::*;
//!
//! // 非同期バリアの使用
//! let barrier = RcAsyncBarrier::new(2);
//! let other = barrier.clone();
//!
//! // カウントダウンラッチの使用
//! let latch = RcCountDownLatch::new(3);
//! latch.count_down().await;
//!
//! // 待機グループの使用
//! let wg = RcWaitGroup::new();
//! wg.add(2);
//! wg.done();
//! wg.wait().await;
//! ```
//!
//! # Embassy との統合
//!
//! このクレートは、[Embassy](https://embassy.dev/) エコシステムと統合されており、
//! `embassy_sync` の同期プリミティブを内部で使用しています。

#![no_std]

extern crate alloc;

pub(crate) mod collections;
pub(crate) mod concurrent;
pub(crate) mod sync;
pub(crate) mod timing;

pub use nexus_utils_core_rs::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, Element,
  MpscHandle, PriorityMessage, QueueBase, QueueError, QueueReader, QueueRw, QueueRwHandle, QueueSize, QueueStorage,
  QueueWriter, RingBackend, RingBuffer, RingQueue, RingStorageBackend, Shared, Stack, StackBackend, StackHandle,
  StackStorage, StackStorageBackend, StateCell, TimerDeadline, DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};

pub use collections::*;
#[cfg(feature = "arc")]
pub use concurrent::{
  ArcAsyncBarrierBackend, ArcCountDownLatchBackend, ArcCsAsyncBarrier, ArcCsCountDownLatch, ArcCsSynchronized,
  ArcCsSynchronizedRw, ArcCsWaitGroup, ArcLocalAsyncBarrier, ArcLocalCountDownLatch, ArcLocalSynchronized,
  ArcLocalSynchronizedRw, ArcLocalWaitGroup, ArcMutexBackend, ArcRwLockBackend, ArcSynchronized, ArcSynchronizedRw,
  ArcWaitGroupBackend,
};
#[cfg(feature = "rc")]
pub use concurrent::{
  RcAsyncBarrier, RcAsyncBarrierBackend, RcCountDownLatch, RcCountDownLatchBackend, RcMutexBackend, RcRwLockBackend,
  RcSynchronized, RcSynchronizedRw, RcWaitGroup, RcWaitGroupBackend,
};
pub use sync::*;
pub use timing::ManualDeadlineTimer;

/// 一般的に使用される型と関数を再エクスポートするプレリュードモジュール
///
/// このモジュールは、`nexus-utils-embedded-rs`の主要な型とトレイトを
/// 一括でインポートするための便利な方法を提供します。
///
/// # 使用例
///
/// ```
/// use nexus_utils_embedded_rs::prelude::*;
///
/// let queue = RcMpscBoundedQueue::new(10);
/// let stack = RcStack::new();
/// ```
pub mod prelude {
  pub use super::ManualDeadlineTimer;
  #[cfg(feature = "arc")]
  pub use super::{
    ArcCsAsyncBarrier, ArcCsCountDownLatch, ArcCsSynchronized, ArcCsSynchronizedRw, ArcCsWaitGroup,
    ArcLocalAsyncBarrier, ArcLocalCountDownLatch, ArcLocalWaitGroup, ArcSynchronized, ArcSynchronizedRw,
  };
  #[cfg(feature = "arc")]
  pub use super::{
    ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcCsPriorityQueue, ArcCsStack, ArcLocalMpscBoundedQueue,
    ArcLocalMpscUnboundedQueue, ArcLocalPriorityQueue, ArcLocalRingQueue, ArcLocalStack, ArcMpscBoundedQueue,
    ArcMpscUnboundedQueue, ArcPriorityQueue, ArcRingQueue, ArcStack,
  };
  #[cfg(feature = "arc")]
  pub use super::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
  #[cfg(feature = "rc")]
  pub use super::{RcAsyncBarrier, RcCountDownLatch, RcSynchronized, RcSynchronizedRw, RcWaitGroup};
  #[cfg(feature = "rc")]
  pub use super::{RcMpscBoundedQueue, RcMpscUnboundedQueue, RcPriorityQueue, RcRingQueue, RcStack};
  #[cfg(feature = "rc")]
  pub use super::{RcShared, RcStateCell};
  pub use nexus_utils_core_rs::{
    DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, Element,
    MpscHandle, PriorityMessage, QueueBase, QueueError, QueueReader, QueueRw, QueueRwHandle, QueueSize, QueueStorage,
    QueueWriter, RingBackend, RingBuffer, RingQueue, RingStorageBackend, Shared, Stack, StackBackend, StackBase,
    StackBuffer, StackError, StackHandle, StackMut, StackStorage, StackStorageBackend, StateCell, TimerDeadline,
    DEFAULT_CAPACITY, DEFAULT_PRIORITY, PRIORITY_LEVELS,
  };
}

#[cfg(test)]
mod tests;
