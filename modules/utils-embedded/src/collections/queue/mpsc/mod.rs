#[cfg(feature = "arc")]
pub mod arc_mpsc_queue;
#[cfg(feature = "rc")]
pub mod rc_mpsc_queue;

#[cfg(feature = "arc")]
pub use arc_mpsc_queue::ArcMpscBoundedQueue;
#[cfg(feature = "arc")]
pub use arc_mpsc_queue::ArcMpscUnboundedQueue;
#[cfg(feature = "arc")]
pub use arc_mpsc_queue::{
  ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcLocalMpscBoundedQueue, ArcLocalMpscUnboundedQueue,
};
#[cfg(feature = "rc")]
pub use rc_mpsc_queue::{RcMpscBoundedQueue, RcMpscUnboundedQueue};
