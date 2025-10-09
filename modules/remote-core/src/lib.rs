//! Core implementation of remote messaging functionality.
//!
//! This crate is a placeholder for future remote messaging logic.
//! The current goal is to provide integration points for `FailureEventStream`.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_clone)]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enum)]
#![deny(clippy::no_effect)]
#![deny(clippy::drop_copy)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::wrong_pub_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream, FailureInfo, FailureMetadata};

/// Handler for notifying failure events from remote actors.
///
/// Uses `FailureEventStream` to distribute failure information and enables additional processing through custom handlers.
#[cfg(feature = "std")]
pub struct RemoteFailureNotifier<E>
where
  E: FailureEventStream, {
  hub: E,
  handler: Option<FailureEventListener>,
}

#[cfg(feature = "std")]
impl<E> RemoteFailureNotifier<E>
where
  E: FailureEventStream,
{
  /// Creates a new `RemoteFailureNotifier`.
  ///
  /// # Arguments
  ///
  /// * `hub` - The stream for distributing failure events
  pub fn new(hub: E) -> Self {
    Self { hub, handler: None }
  }

  /// Gets the event stream listener.
  ///
  /// # Returns
  ///
  /// A `FailureEventListener` instance
  pub fn listener(&self) -> FailureEventListener {
    self.hub.listener()
  }

  /// Gets a reference to the event hub.
  ///
  /// # Returns
  ///
  /// A reference to the `FailureEventStream`
  pub fn hub(&self) -> &E {
    &self.hub
  }

  /// Gets the configured custom handler.
  ///
  /// # Returns
  ///
  /// `Some(&FailureEventListener)` if a handler is set, otherwise `None`
  pub fn handler(&self) -> Option<&FailureEventListener> {
    self.handler.as_ref()
  }

  /// Sets a custom handler.
  ///
  /// # Arguments
  ///
  /// * `handler` - The handler to process failure events
  pub fn set_handler(&mut self, handler: FailureEventListener) {
    self.handler = Some(handler);
  }

  /// Dispatches failure information if a custom handler is configured.
  ///
  /// # Arguments
  ///
  /// * `info` - The failure information
  pub fn dispatch(&self, info: FailureInfo) {
    if let Some(handler) = self.handler.as_ref() {
      handler(FailureEvent::RootEscalated(info.clone()));
    }
  }

  /// Sends failure information to both the event hub and custom handler.
  ///
  /// # Arguments
  ///
  /// * `info` - The failure information
  pub fn emit(&self, info: FailureInfo) {
    self.hub.listener()(FailureEvent::RootEscalated(info.clone()));
    self.dispatch(info);
  }
}

/// Creates placeholder metadata with endpoint information.
///
/// # Arguments
///
/// * `endpoint` - The endpoint name
///
/// # Returns
///
/// `FailureMetadata` configured with the endpoint information
pub fn placeholder_metadata(endpoint: &str) -> FailureMetadata {
  FailureMetadata::new().with_endpoint(endpoint.to_owned())
}

#[cfg(test)]
mod tests;
