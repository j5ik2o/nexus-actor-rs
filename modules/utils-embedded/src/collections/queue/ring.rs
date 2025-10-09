//! リングバッファキュー実装

#[cfg(feature = "arc")]
/// `Arc`ベースのリングバッファキュー
pub mod arc_ring_queue;
#[cfg(feature = "rc")]
/// `Rc`ベースのリングバッファキュー
pub mod rc_ring_queue;

#[cfg(feature = "arc")]
pub use arc_ring_queue::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use rc_ring_queue::RcRingQueue;
