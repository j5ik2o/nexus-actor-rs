#[cfg(feature = "arc")]
pub mod arc_ring_queue;
#[cfg(feature = "rc")]
pub mod rc_ring_queue;

#[cfg(feature = "arc")]
pub use arc_ring_queue::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use rc_ring_queue::RcRingQueue;
