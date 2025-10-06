//! Core-facing API for Nexus remote messaging.

#![cfg_attr(not(feature = "std"), no_std)]

// Core API (no_std + alloc) definitions live here. These abstractions
// decouple transport/runtime concerns from any concrete std implementation.
#[cfg(any(feature = "alloc", feature = "std"))]
pub mod core_api;

// Re-export the core API so downstream crates can depend on this crate
// without referring to the module path explicitly.
#[cfg(any(feature = "alloc", feature = "std"))]
pub use core_api::*;

/// Returns the crate version for diagnostics.
pub fn version() -> &'static str {
  env!("CARGO_PKG_VERSION")
}
