//! Core utilities shared between message derive procedural macros and runtime support.

/// Returns the crate version for diagnostics.
pub fn version() -> &'static str {
  env!("CARGO_PKG_VERSION")
}
