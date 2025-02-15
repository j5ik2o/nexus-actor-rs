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

// Re-exports with explicit paths
pub use actor_context::{
  ActorSystem, Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart, TypedContext, TypedRootContext,
};
pub use context_handle::ContextHandle;
pub use mock_context::MockContext;
pub use receiver_context_handle::ReceiverContextHandle;
pub use root_context::RootContext as RootContextImpl;
pub use sender_context_handle::SenderContextHandle;
pub use spawner_context_handle::SpawnerContextHandle;
pub use typed_actor_context::TypedActorContext;
pub use typed_context_handle::TypedContextHandle;
pub use typed_root_context::TypedRootContext as TypedRootContextImpl;
