//! no_std-friendly queue primitives shared between runtimes.

mod queue_error;
mod queue_size;
mod storage;
mod traits;

mod mpsc;
mod priority;
mod ring;

pub use queue_error::QueueError;
pub use queue_size::QueueSize;
pub use storage::{QueueStorage, RingBufferStorage};
pub use traits::QueueHandle as QueueRwHandle;
pub use traits::{QueueBase, QueueHandle, QueueReader, QueueRw, QueueWriter};

pub use mpsc::{MpscBackend, MpscBuffer, MpscHandle, MpscQueue, RingBufferBackend};
pub use priority::{PriorityMessage, PriorityQueue, DEFAULT_PRIORITY, PRIORITY_LEVELS};
pub use ring::{RingBackend, RingBuffer, RingHandle, RingQueue, RingStorageBackend, DEFAULT_CAPACITY};
