/// Errors that occur during queue operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError<T> {
  /// The queue is full and cannot accept more elements. Contains the element that was attempted to be added.
  Full(T),
  /// The offer operation to the queue failed. Contains the element that was attempted to be provided.
  OfferError(T),
  /// The queue is closed. Contains the element that was attempted to be sent.
  Closed(T),
  /// The connection to the queue has been disconnected.
  Disconnected,
}
