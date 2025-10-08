#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Remote messaging core scaffolding.
//
// This crate is a placeholder for future remote messaging logic.
// Current goal: provide FailureEventStream integration points.

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream, FailureInfo, FailureMetadata};

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
  pub fn new(hub: E) -> Self {
    Self { hub, handler: None }
  }

  pub fn listener(&self) -> FailureEventListener {
    self.hub.listener()
  }

  pub fn hub(&self) -> &E {
    &self.hub
  }

  pub fn handler(&self) -> Option<&FailureEventListener> {
    self.handler.as_ref()
  }

  pub fn set_handler(&mut self, handler: FailureEventListener) {
    self.handler = Some(handler);
  }

  pub fn dispatch(&self, info: FailureInfo) {
    if let Some(handler) = self.handler.as_ref() {
      handler(FailureEvent::RootEscalated(info.clone()));
    }
  }

  pub fn emit(&self, info: FailureInfo) {
    self.hub.listener()(FailureEvent::RootEscalated(info.clone()));
    self.dispatch(info);
  }
}

pub fn placeholder_metadata(endpoint: &str) -> FailureMetadata {
  FailureMetadata::new().with_endpoint(endpoint.to_owned())
}

#[cfg(test)]
mod tests;
