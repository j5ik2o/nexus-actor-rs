//! Foundation module for synchronization primitives.
//!
//! This module provides implementations of shared references and state cells.
//! These are used as the foundation for collections and concurrency primitives.
//!
//! # Provided Types
//!
//! - **RcShared / ArcShared**: Shared reference wrapper (implements `Shared` trait)
//! - **RcStateCell / ArcStateCell**: State cell (implements `StateCell` trait)
//!
//! # Feature Flags
//!
//! - **`rc`**: `Rc`-based implementation (single-threaded only)
//! - **`arc`**: `Arc`-based implementation (multi-threaded support)
//!   - `ArcLocal*`: Optimized implementation using local mutex
//!   - `ArcCs*`: Critical section-based implementation
//!   - `Arc*`: Standard implementation

#[cfg(feature = "arc")]
mod arc;
#[cfg(feature = "rc")]
mod rc;

#[cfg(feature = "arc")]
pub use arc::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "rc")]
pub use rc::{RcShared, RcStateCell};
