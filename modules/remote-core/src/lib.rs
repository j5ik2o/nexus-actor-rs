#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Remote messaging core scaffolding.
//
// This crate is a placeholder for future remote messaging logic.
// Current goal: provide FailureEventHub integration points.

use nexus_actor_core_rs::{FailureEventHub, FailureEventListener, FailureMetadata};

#[cfg(feature = "std")]
pub struct RemoteFailureNotifier {
  hub: FailureEventHub,
}

#[cfg(feature = "std")]
impl RemoteFailureNotifier {
  pub fn new(hub: FailureEventHub) -> Self {
    Self { hub }
  }

  pub fn listener(&self) -> FailureEventListener {
    self.hub.listener()
  }

  pub fn hub(&self) -> &FailureEventHub {
    &self.hub
  }
}

pub fn placeholder_metadata(endpoint: &str) -> FailureMetadata {
  FailureMetadata::new().with_endpoint(endpoint.to_owned())
}
