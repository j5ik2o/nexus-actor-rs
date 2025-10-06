//! Core-facing re-exports for Nexus cluster functionality.

pub use nexus_cluster_std_rs::*;

/// Returns the crate version for diagnostics.
pub fn version() -> &'static str {
  env!("CARGO_PKG_VERSION")
}
