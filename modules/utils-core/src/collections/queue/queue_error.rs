#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError<T> {
  Full(T),
  OfferError(T),
  Closed(T),
  Disconnected,
}
