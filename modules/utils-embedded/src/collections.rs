//! コレクション型モジュール
//!
//! このモジュールは、`no_std`環境で利用可能なキューやスタックなどのコレクション型を提供します。

/// キューコレクション
///
/// MPSC、優先度付き、リングバッファベースのキュー実装を提供します。
pub mod queue;

/// スタックコレクション
///
/// LIFO（後入れ先出し）スタック実装を提供します。
pub mod stack;

#[cfg(feature = "arc")]
pub use queue::mpsc::{
  ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcLocalMpscBoundedQueue, ArcLocalMpscUnboundedQueue,
  ArcMpscBoundedQueue, ArcMpscUnboundedQueue,
};
#[cfg(feature = "rc")]
pub use queue::mpsc::{RcMpscBoundedQueue, RcMpscUnboundedQueue};
#[cfg(feature = "rc")]
pub use queue::priority::RcPriorityQueue;
#[cfg(feature = "arc")]
pub use queue::priority::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use queue::ring::RcRingQueue;
#[cfg(feature = "arc")]
pub use queue::ring::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use stack::RcStack;
#[cfg(feature = "arc")]
pub use stack::{ArcCsStack, ArcLocalStack, ArcStack};
