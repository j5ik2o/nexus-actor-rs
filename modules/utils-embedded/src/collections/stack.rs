//! Stack implementation

#[cfg(feature = "arc")]
/// `Arc`-based stack
pub mod arc_stack;
#[cfg(feature = "rc")]
/// `Rc`-based stack
pub mod rc_stack;

#[cfg(feature = "arc")]
pub use arc_stack::{ArcCsStack, ArcLocalStack, ArcStack};
#[cfg(feature = "rc")]
pub use rc_stack::RcStack;
