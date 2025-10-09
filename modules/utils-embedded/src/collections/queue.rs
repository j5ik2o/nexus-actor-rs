//! キュー実装モジュール

/// MPSC（Multiple Producer, Single Consumer）キュー
///
/// 複数のプロデューサーと単一のコンシューマーをサポートするキュー実装です。
pub mod mpsc;

/// 優先度付きキュー
///
/// メッセージの優先度に基づいて処理順序を制御するキュー実装です。
pub mod priority;

/// リングバッファキュー
///
/// 循環バッファを使用した効率的なFIFOキュー実装です。
pub mod ring;

#[cfg(feature = "arc")]
pub use mpsc::arc_mpsc_bounded_queue::{ArcCsMpscBoundedQueue, ArcLocalMpscBoundedQueue, ArcMpscBoundedQueue};
#[cfg(feature = "arc")]
pub use mpsc::arc_mpsc_unbounded_queue::{ArcCsMpscUnboundedQueue, ArcLocalMpscUnboundedQueue, ArcMpscUnboundedQueue};
#[cfg(feature = "rc")]
pub use mpsc::rc_mpsc_bounded_queue::RcMpscBoundedQueue;
#[cfg(feature = "rc")]
pub use mpsc::rc_mpsc_unbounded_queue::RcMpscUnboundedQueue;
#[cfg(feature = "arc")]
pub use priority::arc_priority_queue::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use priority::rc_priority_queue::RcPriorityQueue;
#[cfg(feature = "arc")]
pub use ring::arc_ring_queue::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use ring::rc_ring_queue::RcRingQueue;
