//! Core-facing API and compatibility re-exports for Nexus remote messaging.

#![cfg_attr(not(feature = "std"), no_std)]

// Core API (no_std + alloc) definitions live here. These are forward-looking
// abstractions to decouple transport/runtime from the std implementation.
// The existing std-based API remains available via re-exports below.
#[cfg(any(feature = "alloc", feature = "std"))]
pub mod core_api;

// Backward-compatible re-export of the std implementation.
// Keep enabled while migrating downstream crates.
#[cfg(feature = "std")]
pub use nexus_remote_std_rs::*;

/// Returns the crate version for diagnostics.
pub fn version() -> &'static str {
  env!("CARGO_PKG_VERSION")
}
