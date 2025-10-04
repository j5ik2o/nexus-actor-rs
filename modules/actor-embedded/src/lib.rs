#![no_std]

extern crate alloc;

pub use nexus_actor_core_rs as core;

#[cfg(feature = "embassy")]
pub mod embedded;
#[cfg(feature = "embassy")]
pub mod spawn;

#[cfg(feature = "embassy")]
pub use embedded::EmbeddedRuntimeBuilder;
#[cfg(feature = "embassy")]
pub use nexus_actor_core_rs::runtime::{FnCoreSpawner, FnJoinHandle};
#[cfg(feature = "embassy")]
pub use spawn::EmbassyScheduler;
