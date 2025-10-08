#[cfg(feature = "arc")]
pub mod arc_priority_queue;
#[cfg(feature = "rc")]
pub mod rc_priority_queue;

#[cfg(feature = "arc")]
pub use arc_priority_queue::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use rc_priority_queue::RcPriorityQueue;
