//! Collection types module
//!
//! This module provides collection types such as queues and stacks that are usable in `no_std` environments.

/// Queue collections
///
/// Provides MPSC, priority-based, and ring buffer-based queue implementations.
pub mod queue;

/// Stack collections
///
/// Provides LIFO (Last-In-First-Out) stack implementations.
pub mod stack;

#[cfg(feature = "arc")]
pub use queue::mpsc::{
  ArcCsMpscBoundedQueue, ArcCsMpscUnboundedQueue, ArcLocalMpscBoundedQueue, ArcLocalMpscUnboundedQueue,
  ArcMpscBoundedQueue, ArcMpscUnboundedQueue,
};
#[cfg(feature = "rc")]
pub use queue::mpsc::{RcMpscBoundedQueue, RcMpscUnboundedQueue};
#[cfg(feature = "rc")]
pub use queue::priority::RcPriorityQueue;
#[cfg(feature = "arc")]
pub use queue::priority::{ArcCsPriorityQueue, ArcLocalPriorityQueue, ArcPriorityQueue};
#[cfg(feature = "rc")]
pub use queue::ring::RcRingQueue;
#[cfg(feature = "arc")]
pub use queue::ring::{ArcLocalRingQueue, ArcRingQueue};
#[cfg(feature = "rc")]
pub use stack::RcStack;
#[cfg(feature = "arc")]
pub use stack::{ArcCsStack, ArcLocalStack, ArcStack};
