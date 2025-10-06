#![no_std]

extern crate alloc;

mod mailbox;
mod spawn;
mod timer;

pub use mailbox::{LocalMailbox, LocalMailboxRecv};
#[cfg(feature = "embedded_arc")]
pub use nexus_utils_embedded_rs::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "embedded_rc")]
pub use nexus_utils_embedded_rs::sync::{RcShared, RcStateCell};
pub use spawn::ImmediateSpawner;
pub use timer::ImmediateTimer;

pub mod prelude {
  #[cfg(feature = "embedded_arc")]
  pub use super::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}
