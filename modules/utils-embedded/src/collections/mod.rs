#[cfg(feature = "arc")]
pub mod mpsc_queue_arc;
#[cfg(feature = "rc")]
pub mod mpsc_queue_rc;
#[cfg(feature = "arc")]
pub mod priority_queue_arc;
#[cfg(feature = "rc")]
pub mod priority_queue_rc;
#[cfg(feature = "arc")]
pub mod queue_arc;
#[cfg(feature = "rc")]
pub mod queue_rc;

#[cfg(feature = "arc")]
pub use mpsc_queue_arc::{
  ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcLocalMpscBoundedQueue, ArcLocalMpscUnboundedQueue,
  ArcMpscBoundedQueue, ArcMpscUnboundedQueue,
};
#[cfg(feature = "rc")]
pub use mpsc_queue_rc::{RcMpscBoundedQueue, RcMpscUnboundedQueue};
#[cfg(feature = "arc")]
pub use priority_queue_arc::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use priority_queue_rc::RcPriorityQueue;
#[cfg(feature = "arc")]
pub use queue_arc::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use queue_rc::RcRingQueue;
