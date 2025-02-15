//! Context module provides actor context functionality.

pub mod actor_context;
pub mod context_handle;
pub mod mock_context;
pub mod receiver_context_handle;
pub mod root_context;
pub mod sender_context_handle;
pub mod spawner_context_handle;
pub mod typed_actor_context;
pub mod typed_context_handle;
pub mod typed_root_context;

// Re-exports
pub use self::{
  actor_context::*, context_handle::*, mock_context::*, receiver_context_handle::*, root_context::*,
  sender_context_handle::*, spawner_context_handle::*, typed_actor_context::*, typed_context_handle::*,
  typed_root_context::*,
};

// Type aliases
pub type ActorSystem = crate::actor::context::actor_context::ActorSystem;
