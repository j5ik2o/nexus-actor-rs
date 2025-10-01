//! Core-facing re-exports for Nexus remote messaging functionality.

pub use nexus_remote_std_rs::*;

/// Returns the crate version for diagnostics.
pub fn version() -> &'static str {
  env!("CARGO_PKG_VERSION")
}
