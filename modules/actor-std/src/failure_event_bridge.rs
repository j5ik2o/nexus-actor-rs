use std::sync::Arc;

use nexus_actor_core_rs::{FailureEvent, FailureEventListener};
use tokio::sync::broadcast;

/// A wrapper that distributes FailureEvent via Tokio broadcast channels.
#[derive(Clone)]
pub struct TokioFailureEventBridge {
  sender: broadcast::Sender<FailureEvent>,
}

impl TokioFailureEventBridge {
  /// Creates a new `TokioFailureEventBridge` instance.
  ///
  /// # Arguments
  ///
  /// * `capacity` - The buffer capacity of the internal broadcast channel
  ///
  /// # Returns
  ///
  /// A new bridge instance with a broadcast channel of the specified capacity
  pub fn new(capacity: usize) -> Self {
    let (sender, _) = broadcast::channel(capacity);
    Self { sender }
  }

  /// Returns a listener that broadcasts failure events.
  ///
  /// This method returns a `FailureEventListener` that receives failure events
  /// and broadcasts them to all subscribers. Send errors are ignored.
  ///
  /// # Returns
  ///
  /// A listener function that broadcasts failure events
  pub fn listener(&self) -> FailureEventListener {
    let sender = self.sender.clone();
    Arc::new(move |event: FailureEvent| {
      let _ = sender.send(event.clone());
    })
  }

  /// Creates a receiver for failure events.
  ///
  /// This method creates a new receiver for the broadcast channel.
  /// The returned receiver can be used to receive failure events
  /// that are broadcast through the bridge.
  ///
  /// # Returns
  ///
  /// A broadcast receiver for receiving failure events
  pub fn subscribe(&self) -> broadcast::Receiver<FailureEvent> {
    self.sender.subscribe()
  }
}
