//! no_std-friendly queue primitives shared between runtimes.

mod ring_buffer;
mod ring_queue_base;

pub use ring_buffer::RingBuffer;
pub use ring_queue_base::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, DEFAULT_CAPACITY};
