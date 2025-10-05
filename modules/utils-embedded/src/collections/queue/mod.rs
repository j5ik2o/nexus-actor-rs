pub mod mpsc;
pub mod priority;
pub mod ring;

#[cfg(feature = "arc")]
pub use mpsc::arc_mpsc_queue::{
  ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcLocalMpscBoundedQueue, ArcLocalMpscUnboundedQueue,
  ArcMpscBoundedQueue, ArcMpscUnboundedQueue,
};
#[cfg(feature = "rc")]
pub use mpsc::rc_mpsc_queue::{RcMpscBoundedQueue, RcMpscUnboundedQueue};
#[cfg(feature = "arc")]
pub use priority::arc_priority_queue::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use priority::rc_priority_queue::RcPriorityQueue;
#[cfg(feature = "arc")]
pub use ring::arc_ring_queue::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use ring::rc_ring_queue::RcRingQueue;
