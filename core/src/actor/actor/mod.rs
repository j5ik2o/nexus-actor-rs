//! Actor module provides core actor functionality.

pub mod actor;
pub mod actor_ref;
pub mod lifecycle;

pub use self::{actor::*, actor_ref::*, lifecycle::*};
