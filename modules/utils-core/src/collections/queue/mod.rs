//! no_std-friendly queue primitives shared between runtimes.

mod queue_error;
mod queue_size;
mod traits;

pub mod mpsc;
pub mod ring;

pub use queue_error::QueueError;
pub use queue_size::QueueSize;
pub use traits::{QueueBase, QueueReader, QueueWriter, SharedQueue};

pub use mpsc::{MpscBackend, MpscBuffer, MpscQueue, RingBufferBackend, RingBufferStorage, SharedMpscHandle};
pub use ring::{QueueStorage, RingBuffer, RingQueue, SharedQueueHandle, DEFAULT_CAPACITY};
