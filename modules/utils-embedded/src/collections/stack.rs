//! スタック実装

#[cfg(feature = "arc")]
/// `Arc`ベースのスタック
pub mod arc_stack;
#[cfg(feature = "rc")]
/// `Rc`ベースのスタック
pub mod rc_stack;

#[cfg(feature = "arc")]
pub use arc_stack::{ArcCsStack, ArcLocalStack, ArcStack};
#[cfg(feature = "rc")]
pub use rc_stack::RcStack;
