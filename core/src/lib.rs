//! Core functionality for the actor system.

pub mod actor;
pub mod event_stream;

pub use actor::*;
pub use event_stream::*;

// Re-exports for derive macros
pub use nexus_actor_message_derive_rs::Message;
