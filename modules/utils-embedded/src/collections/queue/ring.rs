//! Ring buffer queue implementation

#[cfg(feature = "arc")]
/// `Arc`-based ring buffer queue
pub mod arc_ring_queue;
#[cfg(feature = "rc")]
/// `Rc`-based ring buffer queue
pub mod rc_ring_queue;

#[cfg(feature = "arc")]
pub use arc_ring_queue::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use rc_ring_queue::RcRingQueue;
