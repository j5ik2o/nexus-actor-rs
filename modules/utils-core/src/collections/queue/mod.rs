//! no_std-friendly queue primitives shared between runtimes.

pub mod mpsc;
pub mod ring;

pub use mpsc::{MpscBuffer, MpscStorage, SharedMpscHandle, SharedMpscQueue};
pub use ring::{
  QueueBase, QueueError, QueueReader, QueueSize, QueueStorage, QueueWriter, RingBuffer, SharedQueue, SharedQueueHandle,
  SharedRingQueue, DEFAULT_CAPACITY,
};
