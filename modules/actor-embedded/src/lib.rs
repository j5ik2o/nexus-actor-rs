#![no_std]

extern crate alloc;

mod mailbox;
mod shared;
mod spawn;
mod state;
mod timer;

pub use mailbox::{LocalMailbox, LocalMailboxRecv};
#[cfg(feature = "embedded_rc")]
pub use shared::RcShared;
pub use spawn::ImmediateSpawner;
#[cfg(feature = "embedded_rc")]
pub use state::RcStateCell;
#[cfg(feature = "embedded_arc")]
pub use state::{ArcCsStateCell, ArcLocalStateCell, ArcStateCell};
pub use timer::ImmediateTimer;

pub mod prelude {
  #[cfg(feature = "embedded_arc")]
  pub use super::{ArcCsStateCell, ArcLocalStateCell, ArcStateCell};
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}
