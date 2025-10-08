#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Cluster coordination scaffolding.
// Provides placeholder API to wire FailureEventStream into future cluster services.

use nexus_actor_core_rs::{FailureEvent, FailureEventListener, FailureEventStream};
use nexus_remote_core_rs::RemoteFailureNotifier;

#[cfg(feature = "std")]
pub struct ClusterFailureBridge<E>
where
  E: FailureEventStream, {
  hub: E,
  remote_notifier: RemoteFailureNotifier<E>,
}

#[cfg(feature = "std")]
impl<E> ClusterFailureBridge<E>
where
  E: FailureEventStream,
{
  pub fn new(hub: E, remote_notifier: RemoteFailureNotifier<E>) -> Self {
    Self { hub, remote_notifier }
  }

  pub fn register(&self) -> FailureEventListener {
    self.hub.listener()
  }

  pub fn notifier(&self) -> &RemoteFailureNotifier<E> {
    &self.remote_notifier
  }

  pub fn fan_out(&self, event: FailureEvent) {
    let FailureEvent::RootEscalated(info) = &event;
    self.remote_notifier.dispatch(info.clone());
    self.hub.listener()(event);
  }
}

#[cfg(test)]
mod tests;
