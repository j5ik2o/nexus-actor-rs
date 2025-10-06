#![cfg(feature = "alloc")]

pub mod core;
pub mod extensions;
pub mod middleware;
pub mod snapshot;

pub use core::{
  CoreActorContext, CoreActorContextBuilder, CoreActorContextSnapshot, CoreActorSystemId, CoreMailboxFactory,
  CoreProps, CorePropsFactory, CoreReceiverInvocation, CoreSenderInvocation, CoreSenderSnapshot,
  CoreSupervisorStrategyHandle,
};
pub use extensions::{
  next_core_context_extension_id, CoreContextExtension, CoreContextExtensionHandle, CoreContextExtensionId,
  CoreContextExtensions, CoreExtensionBorrow, CoreExtensionBorrowMut,
};
pub use middleware::{
  compose_receiver_chain, compose_sender_chain, compose_spawn_chain, CoreReceiverMiddleware,
  CoreReceiverMiddlewareChain, CoreSenderMiddleware, CoreSenderMiddlewareChain, CoreSpawnMiddleware,
  CoreSpawnMiddlewareChain,
};
pub use snapshot::{CoreContextSnapshot, CoreReceiverSnapshot};
