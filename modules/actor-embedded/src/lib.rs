#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub use nexus_actor_core_rs as core;

#[cfg(feature = "embassy")]
pub mod embedded;
#[cfg(feature = "embassy")]
pub mod spawn;

#[cfg(all(test, feature = "embassy"))]
mod tests;

#[cfg(feature = "embassy")]
pub use embedded::EmbeddedRuntimeBuilder;
#[cfg(feature = "embassy")]
pub use nexus_actor_core_rs::runtime::{FnCoreSpawner, FnJoinHandle};
#[cfg(feature = "embassy")]
pub use spawn::EmbassyScheduler;
