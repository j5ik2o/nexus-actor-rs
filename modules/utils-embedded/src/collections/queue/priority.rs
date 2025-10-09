//! 優先度付きキュー実装

#[cfg(feature = "arc")]
/// `Arc`ベースの優先度付きキュー
pub mod arc_priority_queue;
#[cfg(feature = "rc")]
/// `Rc`ベースの優先度付きキュー
pub mod rc_priority_queue;

#[cfg(feature = "arc")]
pub use arc_priority_queue::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use rc_priority_queue::RcPriorityQueue;
