#[cfg(feature = "arc")]
pub mod arc_mpsc_bounded_queue;
#[cfg(feature = "arc")]
pub mod arc_mpsc_unbounded_queue;
#[cfg(feature = "rc")]
pub mod rc_mpsc_bounded_queue;
#[cfg(feature = "rc")]
pub mod rc_mpsc_unbounded_queue;

#[cfg(feature = "arc")]
pub use arc_mpsc_bounded_queue::{ArcCsMpscBoundedQueue, ArcLocalMpscBoundedQueue, ArcMpscBoundedQueue};
#[cfg(feature = "arc")]
pub use arc_mpsc_unbounded_queue::{ArcCsMpscUnboundedQueue, ArcLocalMpscUnboundedQueue, ArcMpscUnboundedQueue};
#[cfg(feature = "rc")]
pub use rc_mpsc_bounded_queue::RcMpscBoundedQueue;
#[cfg(feature = "rc")]
pub use rc_mpsc_unbounded_queue::RcMpscUnboundedQueue;
