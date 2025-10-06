//! no_std-friendly queue primitives shared between runtimes.

mod queue_error;
mod queue_size;
mod storage;
mod traits;

pub mod mpsc;
pub mod ring;

pub use queue_error::QueueError;
pub use queue_size::QueueSize;
pub use storage::{QueueStorage, RingBufferStorage};
pub use traits::QueueHandle as SharedQueueHandle;
pub use traits::{QueueBase, QueueHandle, QueueReader, QueueWriter, SharedQueue};

pub use mpsc::{MpscBackend, MpscBuffer, MpscQueue, RingBufferBackend, SharedMpscHandle};
pub use ring::{RingBuffer, RingQueue, DEFAULT_CAPACITY};
