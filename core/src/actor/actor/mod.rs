//! Actor module provides core actor functionality.

pub mod actor;
pub mod actor_ref;
pub mod lifecycle;

pub use self::{actor::Actor, actor_ref::ActorRef, lifecycle::Lifecycle};
