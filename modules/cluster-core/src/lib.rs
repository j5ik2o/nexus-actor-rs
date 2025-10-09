//! Core implementation of cluster coordination functionality.
//!
//! Provides placeholder APIs for integrating `FailureEventStream` into future cluster services.

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

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream};
use nexus_remote_core_rs::RemoteFailureNotifier;

/// Bridge connecting cluster and remote failure notifications.
///
/// Propagates local failure events to remote nodes, enabling cluster-wide sharing of failure information.
#[cfg(feature = "std")]
pub struct ClusterFailureBridge<E>
where
  E: FailureEventStream,
{
  hub: E,
  remote_notifier: RemoteFailureNotifier<E>,
}

#[cfg(feature = "std")]
impl<E> ClusterFailureBridge<E>
where
  E: FailureEventStream,
{
  /// Creates a new `ClusterFailureBridge`.
  ///
  /// # Arguments
  ///
  /// * `hub` - The local failure event hub
  /// * `remote_notifier` - The notification handler for remote nodes
  pub fn new(hub: E, remote_notifier: RemoteFailureNotifier<E>) -> Self {
    Self { hub, remote_notifier }
  }

  /// Registers a failure event listener.
  ///
  /// # Returns
  ///
  /// A `FailureEventListener` instance
  pub fn register(&self) -> FailureEventListener {
    self.hub.listener()
  }

  /// Gets a reference to the remote failure notification handler.
  ///
  /// # Returns
  ///
  /// A reference to `RemoteFailureNotifier`
  pub fn notifier(&self) -> &RemoteFailureNotifier<E> {
    &self.remote_notifier
  }

  /// Distributes failure events to both local and remote destinations.
  ///
  /// # Arguments
  ///
  /// * `event` - The failure event to distribute
  pub fn fan_out(&self, event: FailureEvent) {
    let FailureEvent::RootEscalated(info) = &event;
    self.remote_notifier.dispatch(info.clone());
    self.hub.listener()(event);
  }
}

#[cfg(test)]
mod tests;
