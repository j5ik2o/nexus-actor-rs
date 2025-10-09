//! Queue implementation module

/// MPSC (Multiple Producer, Single Consumer) queue
///
/// Queue implementation that supports multiple producers and a single consumer.
pub mod mpsc;

/// Priority queue
///
/// Queue implementation that controls processing order based on message priority.
pub mod priority;

/// Ring buffer queue
///
/// Efficient FIFO queue implementation using circular buffers.
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
