#[cfg(feature = "arc")]
pub mod queue_arc;
#[cfg(feature = "rc")]
pub mod queue_rc;

#[cfg(feature = "arc")]
pub use queue_arc::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use queue_rc::RcRingQueue;
