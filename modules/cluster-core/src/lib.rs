#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Cluster coordination scaffolding.
// Provides placeholder API to wire FailureEventHub into future cluster services.

use nexus_actor_core_rs::{FailureEventHub, FailureEventListener};
use nexus_remote_core_rs::RemoteFailureNotifier;

#[cfg(feature = "std")]
pub struct ClusterFailureBridge {
  hub: FailureEventHub,
  remote_notifier: RemoteFailureNotifier,
}

#[cfg(feature = "std")]
impl ClusterFailureBridge {
  pub fn new(hub: FailureEventHub, remote_notifier: RemoteFailureNotifier) -> Self {
    Self { hub, remote_notifier }
  }

  pub fn register(&self) -> FailureEventListener {
    self.hub.listener()
  }

  pub fn notifier(&self) -> &RemoteFailureNotifier {
    &self.remote_notifier
  }
}
